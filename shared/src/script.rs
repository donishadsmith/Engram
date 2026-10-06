use crate::{
    EmulatorId, EmulatorState, ScriptTarget,
    enums::{FetchSource, Width},
    input::constants::DEFAULT_GBA_KEYS,
};
use mlua::{
    Function, HookTriggers, Lua, Table, Thread, Value, Variadic, VmState, thread::ThreadStatus,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    mem::take,
    time::{Duration, Instant},
};

// https://github.com/ioncodes/gecko/blob/master/crates/scripting/src/lib.rs

const HELP: &str = r#"
Memory access through bus:
read_u8(address)  read_u16(address)  read_u32(address)
write_u8(address, value)  write_u16(address, value)  write_u32(address, value)

Memory domains:
memory_domain_names(): table of names
read_domain(name, offset)
write_domain(name, offset, value)
address_to_domain: returns (name, offset)

CPU registers:
cpu_register_names(): table of names
read_cpu_register(name)
write_cpu_register(name, value)

Colors:
to_rgb_hex(value)

Breakpoints:
set_breakpoint(address, {pause = true})
remove_breakpoint(address)
clear_all_breakpoints()
check_breakpoints()

Watchpoints for bus accesses:
set_watchpoint(address, {pause = true, on = "rw", width = "byte", target=None})
  - on: "read", "write", "rw", "change" (change = write of a value different from the last write)
  - width: "byte", "halfword", "word" (gameboy: byte only; halfword/word addresses are aligned for gba)
    a watchpoint only fires for specified access width
  - target: on writes its the incoming value being written to address and on reads its the current value
    at the address being accessed; watchpoint only fires for specified target

remove_watchpoint(address)
clear_all_watchpoints()
check_watchpoints()

Emulator controls:
pause()  resume()  step_instruction()  step_frame()
reset() screenshot()  start_gif()  stop_gif()

Input:
available_inputs(): table of input buttons
set_inputs({a = true, up = true}): buttons held permanantly until released, keyboard/gamepad is ignored;
  set_inputs({}), set_inputs(nil), or set_inputs() to return control to keyboard/gamepad
check_inputs(): prints buttons currently held

Misc:
elapsed_cpu_cycles(): GB/GBC reports t-cycles

Hooks:
function on_frame(): runs once per frame
function on_breakpoint(address): runs when a breakpoint is hit
function on_watchpoint(hit): runs once per watchpoint hit; hit is a table with
  address, value, pc, on, access, pause
"#;

pub enum DomainError {
    UnknownDomain,
    OutOfRange { size: usize },
}

pub enum CpuError {
    UnknownRegister,
    ReadOnly,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScriptRequest {
    Pause,
    Screenshot,
    StartGif,
    StopGif,
    Reset,
    StepInstruction,
    StepFrame,
    Resume,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WatchpointType {
    Read,
    Write,
    ReadWrite,
    Change,
}

impl WatchpointType {
    pub fn from_string(string: String) -> Result<WatchpointType, mlua::Error> {
        match string.to_lowercase().as_str() {
            "read" => Ok(WatchpointType::Read),
            "write" => Ok(WatchpointType::Write),
            "rw" => Ok(WatchpointType::ReadWrite),
            "change" => Ok(WatchpointType::Change),
            _ => Err(mlua::Error::runtime(format!(
                "invalid option for `on`: {string}; valid options are 'read', 'write', 'rw', and 'change'."
            ))),
        }
    }

    pub fn to_string(self) -> &'static str {
        match self {
            WatchpointType::Read => "read",
            WatchpointType::Write => "write",
            WatchpointType::ReadWrite => "rw",
            WatchpointType::Change => "change",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WatchpointArgs {
    pub pause: bool,
    pub on: WatchpointType,
    pub target: Option<u32>,
    pub width: Width,
    pub last_written: Option<u32>,
}

impl WatchpointArgs {
    pub fn write_changed(&mut self, new_value: u32) -> bool {
        let changed = self.last_written != Some(new_value);
        self.last_written = Some(new_value);

        changed
    }

    pub fn fires_on_read(&self, value: u32) -> bool {
        matches!(self.on, WatchpointType::Read | WatchpointType::ReadWrite)
            && self
                .target
                .map_or(true, |target_value| target_value == value)
    }

    pub fn should_fire_on_write(&mut self, value: u32) -> bool {
        let changed = self.write_changed(value);

        let kind_hit = match self.on {
            WatchpointType::Change => changed,
            WatchpointType::Write | WatchpointType::ReadWrite => true,
            WatchpointType::Read => false,
        };

        kind_hit
            && self
                .target
                .map_or(true, |target_value| target_value == value)
    }
}

#[derive(Clone, Copy)]
pub struct WatchpointHit {
    pub address: u32,
    pub value: u32,
    pub pause: bool,
    pub width: Width,
    pub on: WatchpointType,
    pub pc: u32,
    pub fetch_source: Option<FetchSource>,
}

impl WatchpointHit {
    pub const MAX_PENDING: usize = 4096;
}

fn cpu_error(name: &str, error: CpuError, valid: &[&str]) -> mlua::Error {
    match error {
        CpuError::UnknownRegister => mlua::Error::runtime(format!(
            "unknown CPU register '{name}' (valid: {})",
            valid.join(", ")
        )),
        CpuError::ReadOnly => mlua::Error::runtime(format!("CPU register '{name}' is read-only")),
    }
}

fn domain_error(domain: &str, offset: usize, error: DomainError, valid: &[&str]) -> mlua::Error {
    match error {
        DomainError::UnknownDomain => mlua::Error::runtime(format!(
            "unknown memory domain '{domain}' (valid: {})",
            valid.join(", ")
        )),
        DomainError::OutOfRange { size } => mlua::Error::runtime(format!(
            "offset {offset:#x} is out of range for '{domain}' (size {size:#x})"
        )),
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Integer(number) if *number >= 0 => format!("{number} (0x{number:x})"),
        _ => value
            .to_string()
            .unwrap_or_else(|err| mlua::Error::runtime(format!("{err}")).to_string()),
    }
}

fn check_address(emulator_id: EmulatorId, address: u32) -> Result<(), mlua::Error> {
    if emulator_id == EmulatorId::Gb && address > 0xFFFF {
        return Err(mlua::Error::runtime(format!(
            "address {address:#x} out of range: gameboy addresses are 16-bit (max 0xFFFF)"
        )));
    }

    Ok(())
}

fn parse_watchpoint_args(
    emulator_id: EmulatorId,
    address: u32,
    kwargs: Option<Table>,
) -> Result<(u32, WatchpointArgs), mlua::Error> {
    let (pause, on, width, target) = match kwargs {
        Some(table) => (
            table.get::<Option<bool>>("pause")?,
            table.get::<Option<String>>("on")?,
            table.get::<Option<String>>("width")?,
            table.get::<Option<u32>>("target")?,
        ),
        None => (None, None, None, None),
    };

    let pause = pause.unwrap_or(true);
    let on = WatchpointType::from_string(on.unwrap_or_else(|| "rw".to_string()))?;
    let width = Width::from_string(width.unwrap_or_else(|| "byte".to_string()))?;

    if emulator_id == EmulatorId::Gb && width != Width::Byte {
        return Err(mlua::Error::runtime(
            "invalid option for `width`: gameboy only allows byte width",
        ));
    }

    let watchpoint_args = WatchpointArgs {
        pause,
        on,
        target,
        width,
        last_written: None,
    };

    Ok((width.align(address), watchpoint_args))
}

fn get_available_inputs(emulator_id: EmulatorId) -> Vec<String> {
    match emulator_id {
        EmulatorId::Gb | EmulatorId::Gba => {
            let keys = DEFAULT_GBA_KEYS
                .iter()
                .map(|&binding| binding.label.to_string().to_lowercase())
                .collect::<Vec<String>>();

            if emulator_id == EmulatorId::Gb {
                keys[..8].to_vec()
            } else {
                keys
            }
        }
    }
}

fn map_inputs(emulator_id: EmulatorId, kwargs: Option<Table>) -> Option<Vec<bool>> {
    match kwargs {
        Some(table) => {
            let mut key_mask: Vec<bool> = Vec::new();
            let keys = get_available_inputs(emulator_id);
            for key in keys.into_iter() {
                let value = table.get::<Option<bool>>(key.as_str()).ok()?;
                key_mask.push(value.unwrap_or(false))
            }

            Some(key_mask)
        }
        None => None,
    }
}

fn bool_to_keys(emulator_id: EmulatorId, key_mask: &[bool]) -> Vec<String> {
    let keys = get_available_inputs(emulator_id);

    keys.iter()
        .zip(key_mask)
        .filter_map(|(key, &bool)| if bool { Some(key.clone()) } else { None })
        .collect()
}

pub struct ScriptEngine {
    lua: Lua,
    pending: VecDeque<String>,
    output: Vec<String>,
    running: Option<Thread>,
    requests: Vec<ScriptRequest>,
    step_done: bool,
    stop_processes: bool,
}

impl ScriptEngine {
    pub fn new() -> Self {
        //https://docs.rs/mlua/latest/mlua/struct.Thread.html
        let lua = Lua::new();
        let _ = lua
            .load(
                r#"
            function step_instruction()
                _step_instruction()
                coroutine.yield()
            end

            function step_frame()
                _step_frame()
                coroutine.yield()
            end
        "#,
            )
            .exec()
            .unwrap();

        Self {
            lua,
            pending: VecDeque::new(),
            output: Vec::new(),
            running: None,
            requests: Vec::new(),
            step_done: false,
            stop_processes: false,
        }
    }

    pub fn load(&mut self, code: String) {
        // in event some pause feature is added to a future emu
        // dont push multiple times
        if self.pending.back() != Some(&code) {
            self.pending.push_back(code)
        }
    }

    pub fn take_output(&mut self) -> Vec<String> {
        take(&mut self.output)
    }

    pub fn take_requests(&mut self) -> Vec<ScriptRequest> {
        take(&mut self.requests)
    }

    pub fn step_completed(&mut self) {
        self.step_done = true;
    }

    pub fn stop_all_processes(&mut self) {
        self.step_completed();
        self.running = None;
        self.stop_processes = true;
    }

    fn kill_processes(&mut self, target: &mut dyn ScriptTarget) {
        if take(&mut self.stop_processes) {
            let _ = self.lua.globals().set("on_frame", Value::Nil);
            let _ = self.lua.globals().set("on_breakpoint", Value::Nil);
            let _ = self.lua.globals().set("on_watchpoint", Value::Nil);
            target.clear_all_breakpoints();
            target.clear_all_watchpoints();
            target.take_breakpoint_hit();
            target.take_watchpoint_hits();

            self.output.push(
                "all running hooks, watchpoints, and breakpoints have been terminated".to_string(),
            );
        }
    }

    pub fn execute(
        &mut self,
        target: &mut dyn ScriptTarget,
        emulator_id: EmulatorId,
        frame_boundary: bool,
    ) {
        self.kill_processes(target);

        let Self {
            lua,
            pending,
            output,
            running,
            requests,
            step_done,
            stop_processes: _,
        } = self;

        let target = RefCell::new(target);
        let output = RefCell::new(output);
        let requests = RefCell::new(requests);

        let result = lua.scope(|scope| {
            let print = scope.create_function(|_, vals: Variadic<Value>| {
                let line = vals
                    .iter()
                    .map(|val| val.to_string())
                    .collect::<mlua::Result<Vec<String>>>()?
                    .join("\t");

                output.borrow_mut().push(line);
                Ok(())
            })?;
            lua.globals().set("print", print)?;

            let help = scope.create_function(|_, (): ()| {
                for line in HELP.lines() {
                    output.borrow_mut().push(line.to_string());
                }
                Ok(())
            })?;
            lua.globals().set("help", help)?;

            let read_u8 = scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u8(address)))?;
            lua.globals().set("read_u8", read_u8)?;

            let read_u16 = scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u16(address)))?;
            lua.globals().set("read_u16", read_u16)?;

            let read_u32 = scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u32(address)))?;
            lua.globals().set("read_u32", read_u32)?;

            let write_u8 = scope.create_function(|_, (address, value): (u32, u8)| {
                target.borrow_mut().write_u8(address, value);
                Ok(())
            })?;
            lua.globals().set("write_u8", write_u8)?;

            let write_u16 = scope.create_function(|_, (address, value): (u32, u16)| {
                target.borrow_mut().write_u16(address, value);
                Ok(())
            })?;
            lua.globals().set("write_u16", write_u16)?;

            let write_u32 = scope.create_function(|_, (address, value): (u32, u32)| {
                target.borrow_mut().write_u32(address, value);
                Ok(())
            })?;
            lua.globals().set("write_u32", write_u32)?;

            let memory_domain_names = scope
                .create_function(|_, (): ()| Ok(target.borrow().memory_domain_names().to_vec()))?;
            lua.globals()
                .set("memory_domain_names", memory_domain_names)?;

            let read_domain = scope.create_function(|_, (domain, offset): (String, usize)| {
                let result = target.borrow().read_domain(&domain, offset);
                result.map_err(|err| {
                    domain_error(&domain.to_lowercase(), offset, err, target.borrow().memory_domain_names())
                })
            })?;
            lua.globals().set("read_domain", read_domain)?;

            let address_to_domain = scope.create_function(|_, address: u32| {
                target.borrow().address_to_domain(address).ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "address {address:#010x} not mapped to any memory domain"
                    ))
                })
            })?;
            lua.globals().set("address_to_domain", address_to_domain)?;

            let write_domain =
                scope.create_function(|_, (domain, offset, value): (String, usize, u8)| {
                    let result = target.borrow_mut().write_domain(&domain, offset, value);
                    result.map_err(|err| {
                        domain_error(&domain.to_lowercase(), offset, err, target.borrow().memory_domain_names())
                    })
                })?;
            lua.globals().set("write_domain", write_domain)?;

            let cpu_register_names = scope
                .create_function(|_, (): ()| Ok(target.borrow().cpu_register_names().to_vec()))?;
            lua.globals()
                .set("cpu_register_names", cpu_register_names)?;

            let read_cpu_register = scope.create_function(|_, name: String| {
                let name = name.to_lowercase();
                let result = target.borrow().read_cpu_register(name.clone());
                result.map_err(|err| cpu_error(&name, err, target.borrow().cpu_register_names()))
            })?;
            lua.globals().set("read_cpu_register", read_cpu_register)?;

            let write_cpu_register = scope.create_function(|_, (name, value): (String, u32)| {
                let name = name.to_lowercase();
                let result = target.borrow_mut().write_cpu_register(name.clone(), value);
                result.map_err(|err| cpu_error(&name, err, target.borrow().cpu_register_names()))
            })?;
            lua.globals()
                .set("write_cpu_register", write_cpu_register)?;

            let to_rgb_hex = scope.create_function(|_, value: u32| {
                let [r, g, b] = target.borrow().to_rgb(value);
                Ok(format!("#{r:02x}{g:02x}{b:02x}"))
            })?;
            lua.globals().set("to_rgb_hex", to_rgb_hex)?;

            let available_inputs = scope.create_function(|_,  (): ()| {
                Ok(get_available_inputs(emulator_id))
            })?;
            lua.globals().set("available_inputs", available_inputs)?;

            let set_inputs = scope.create_function(|_, kwargs: Option<Table>| {
                let mask = map_inputs(emulator_id, kwargs);
                target.borrow_mut().set_scripted_inputs(mask);

                Ok(())
            })?;
            lua.globals().set("set_inputs", set_inputs)?;

            let check_inputs = scope.create_function(|_,  (): ()| {
                let message = match target.borrow().check_scripted_inputs() {
                    Some(key_mask) => format!("The following keys are set: {}", bool_to_keys(emulator_id, key_mask).join(", ")),
                    None => "No keys set".to_string()
                };

                output.borrow_mut().push(message);

                Ok(())
            })?;
            lua.globals().set("check_inputs", check_inputs)?;

            let set_breakpoint =
                scope.create_function(|_, (address, kwargs): (u32, Option<Table>)| {
                    check_address(emulator_id, address)?;

                    let pause = match kwargs {
                        Some(table) => table.get::<Option<bool>>("pause")?.unwrap_or(true),
                        None => true,
                    };
                    let message = if target.borrow_mut().set_breakpoint(address, pause) {
                        format!("breakpoint set at {address:08x}")
                    } else {
                        format!("breakpoint at {address:08x} was updated")
                    };

                    output.borrow_mut().push(message);
                    Ok(())
                })?;
            lua.globals().set("set_breakpoint", set_breakpoint)?;

            let remove_breakpoint = scope.create_function(|_, address: u32| {
                target.borrow_mut().remove_breakpoint(address);
                output
                    .borrow_mut()
                    .push(format!("breakpoint at {address:08x} removed"));
                Ok(())
            })?;
            lua.globals().set("remove_breakpoint", remove_breakpoint)?;

            let clear_all_breakpoints = scope.create_function(|_, (): ()| {
                target.borrow_mut().clear_all_breakpoints();
                output
                    .borrow_mut()
                    .push("all breakpoints cleared".to_string());
                Ok(())
            })?;
            lua.globals()
                .set("clear_all_breakpoints", clear_all_breakpoints)?;

            let check_breakpoints = scope.create_function(|_, (): ()| {
                let breakpoints = target.borrow_mut().check_breakpoints();
                if !breakpoints.is_empty() {
                    for (address, state) in breakpoints.iter() {
                        let action = if *state == EmulatorState::Paused {
                            "pause"
                        } else {
                            "run"
                        };

                        output.borrow_mut().push(format!(
                            "breakpoint at {:08x}, action on breakpoint: {action}",
                            *address
                        ))
                    }
                } else {
                    output.borrow_mut().push("no breakpoints found".to_string())
                }
                Ok(())
            })?;
            lua.globals().set("check_breakpoints", check_breakpoints)?;

            let elapsed_cpu_cycles = scope.create_function(|_, (): ()| {
                        Ok(target.borrow().elapsed_cpu_cycles())
            })?;
            lua.globals().set("elapsed_cpu_cycles", elapsed_cpu_cycles)?;

            let set_watchpoint =
                scope.create_function(|_, (address, kwargs): (u32, Option<Table>)| {
                    check_address(emulator_id, address)?;

                    let (address, watchpoint_args) =
                        parse_watchpoint_args(emulator_id, address, kwargs)?;
                    let message = if target.borrow_mut().set_watchpoint(address, watchpoint_args) {
                        format!("watchpoint set at {address:08x}")
                    } else {
                        format!("watchpoint at {address:08x} was updated")
                    };

                    output.borrow_mut().push(message);
                    Ok(())
                })?;
            lua.globals().set("set_watchpoint", set_watchpoint)?;

            let remove_watchpoint = scope.create_function(|_, address: u32| {
                target.borrow_mut().remove_watchpoint(address);
                output
                    .borrow_mut()
                    .push(format!("watchpoint at {address:08x} removed"));
                Ok(())
            })?;
            lua.globals().set("remove_watchpoint", remove_watchpoint)?;

            let clear_all_watchpoints = scope.create_function(|_, (): ()| {
                target.borrow_mut().clear_all_watchpoints();
                output
                    .borrow_mut()
                    .push("all watchpoints cleared".to_string());
                Ok(())
            })?;
            lua.globals()
                .set("clear_all_watchpoints", clear_all_watchpoints)?;

            let check_watchpoints = scope.create_function(|_, (): ()| {
                let watchpoints = target.borrow_mut().check_watchpoints();
                if !watchpoints.is_empty() {
                    for (address, watchpoint_args) in watchpoints.iter() {
                        let action = if watchpoint_args.pause {
                            "pause"
                        } else {
                            "run"
                        };
                        let on = watchpoint_args.on.to_string();
                        let width = watchpoint_args.width.to_string();
                        let target_message = if let Some(target) = watchpoint_args.target {

                        format!(" with target={}", target)} else {
                            "".to_string()
                        };

                        output.borrow_mut().push(format!(
                            "watchpoint at {:08x} on {on} for {width} access{}, action on watchpoint: {action}",
                            *address, target_message
                        ))
                    }
                } else {
                    output.borrow_mut().push("no watchpoints found".to_string())
                }
                Ok(())
            })?;
            lua.globals().set("check_watchpoints", check_watchpoints)?;

            let control =
                |name: &'static str, request: fn() -> ScriptRequest, message: &'static str| {
                    let requests = &requests;
                    let output = &output;
                    let function = scope.create_function(move |_, (): ()| {
                        requests.borrow_mut().push(request());

                        if !message.is_empty() {
                            output.borrow_mut().push(message.to_string());
                        }

                        Ok(())
                    })?;
                    lua.globals().set(name, function)
                };

            control("pause", || ScriptRequest::Pause, "emulator paused")?;
            control("resume", || ScriptRequest::Resume, "emulator resumed")?;
            control("_step_instruction", || ScriptRequest::StepInstruction, "")?;
            control("_step_frame", || ScriptRequest::StepFrame, "")?;
            control("reset", || ScriptRequest::Reset, "")?;
            control(
                "screenshot",
                || ScriptRequest::Screenshot,
                "screenshot taken",
            )?;
            control(
                "start_gif",
                || ScriptRequest::StartGif,
                "started gif recording",
            )?;
            control(
                "stop_gif",
                || ScriptRequest::StopGif,
                "stopped gif recording",
            )?;

            if *step_done && let Some(thread) = running.take() {
                *step_done = false;
                time_limit(lua, Duration::from_millis(100));
                match thread.resume::<mlua::MultiValue>(()) {
                    Ok(_) if thread.status() == ThreadStatus::Resumable => *running = Some(thread),
                    Ok(values) if !values.is_empty() => output.borrow_mut().push(
                        values
                            .iter()
                            .map(display_value)
                            .collect::<Vec<_>>()
                            .join("\t")),
                    Ok(_) => {},
                    Err(err) => output.borrow_mut().push(format!("{err}")),
                }
            }

            while running.is_none() && let Some(code) = pending.pop_front() {
                time_limit(lua, Duration::from_millis(100));
                let start = lua.load(&code).into_function().and_then(|function| lua.create_thread(function));
                match start.and_then(|thread| thread.resume::<mlua::MultiValue>(()).map(|value| (thread, value))) {
                    Ok((thread, _)) if thread.status() == ThreadStatus::Resumable => *running = Some(thread),
                    Ok((_, values)) if !values.is_empty() => output.borrow_mut().push(
                        values
                            .iter()
                            .map(display_value)
                            .collect::<Vec<_>>()
                            .join("\t"),
                    ),
                    Ok((_, _)) => {},
                    Err(err) => output.borrow_mut().push(format!("{err}")),
                }
            };

            if frame_boundary && let Ok(hook) = lua.globals().get::<Function>("on_frame") {
                time_limit(lua, Duration::from_millis(5));
                if let Err(err) = hook.call::<()>(()) {
                    output.borrow_mut().push(format!("{err}"));
                    let _ = lua.globals().set("on_frame", Value::Nil);
                }
            }

            // yeah default behavior when no hook is less annoying
            let hit = target.borrow_mut().take_breakpoint_hit();
            if let Some(address) = hit {
                if let Ok(hook) = lua.globals().get::<Function>("on_breakpoint") {
                    time_limit(lua, Duration::from_millis(5));
                    if let Err(err) = hook.call::<()>(address) {
                        output.borrow_mut().push(format!("{err}"));
                    }
                } else {
                    output
                    .borrow_mut()
                    .push(format!("breakpoint at {address:08x} reached"));
                }
            }

            let hits = target.borrow_mut().take_watchpoint_hits();
            if !hits.is_empty() {
                let hook = lua.globals().get::<Function>("on_watchpoint").ok();

                for hit in hits {
                    if hook.is_none() {
                        output.borrow_mut().push(format!(
                            "watchpoint at {:08x} hit: {} {} value={:x} pc={:08x}",
                            hit.address,
                            hit.on.to_string(),
                            hit.width.to_string(),
                            hit.value,
                            hit.pc
                        ));
                    }

                    let Some(hook) = &hook else {
                        continue;
                    };

                    let table = lua.create_table()?;
                    table.set("address", hit.address)?;
                    table.set("value", hit.value)?;
                    table.set("pc", hit.pc)?;
                    table.set("on", hit.on.to_string())?;
                    table.set("width", hit.width.to_string())?;
                    table.set("pause", hit.pause)?;

                    let domain = target.borrow().address_to_domain(hit.address);
                    if let Some((domain, offset)) = domain {
                        table.set("domain", domain)?;
                        table.set("offset", offset)?;
                    }

                    let domain = target.borrow().address_to_domain(hit.pc);
                    if let Some((_, offset)) = domain{
                        table.set("pc_offset", offset)?;
                    }

                    time_limit(lua, Duration::from_millis(5));
                    if let Err(err) = hook.call::<()>(table) {
                        output.borrow_mut().push(format!("{err}"));
                        let _ = lua.globals().set("on_watchpoint", Value::Nil);
                        break;
                    }
                }
            }

            Ok(())
        });

        if let Err(e) = result {
            output
                .borrow_mut()
                .push(format!("script engine error: {e}"));
        }
    }
}

fn time_limit(lua: &Lua, budget: Duration) {
    let start = Instant::now();

    let _ = lua.set_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            if start.elapsed() > budget {
                Err(mlua::Error::runtime(
                    "script ran too long and was terminated",
                ))
            } else {
                Ok(VmState::Continue)
            }
        },
    );
}
