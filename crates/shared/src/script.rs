use crate::{
    EmulatorId, EmulatorState, ScriptTarget,
    constants::SAVE_STATE_SLOTS,
    enums::{FetchSource, Width},
    input::constants::DEFAULT_GBA_KEYS,
};
use mlua::{
    Function, HookTriggers, Lua, Table, Thread, Value, Variadic, VmState, thread::ThreadStatus,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    mem::take,
    time::{Duration, Instant},
};

// https://github.com/ioncodes/gecko/blob/master/crates/scripting/src/lib.rs

pub const API: &str = r#"
Console:
print(...)  console.log(...): intentionally redundent, tab-separated print functions
console.clear()

Memory (bus accesses):
memory.read_u8(address)  memory.read_u16(address)  memory.read_u32(address)
memory.write_u8(address, value)  memory.write_u16(address, value)  memory.write_u32(address, value)

Memory Domains:
memory.domain_names(): table of names
memory.read_domain(name, offset)
memory.write_domain(name, offset, value)
memory.address_to_domain(address): returns (name, offset)

CPU Registers:
cpu.register_names(): table of names
cpu.read_register(name)
cpu.write_register(name, value)

Colors:
color.to_rgb_hex(value)

Controller:
controller.names(): table of input buttons
controller.set({a = true, up = true}): buttons held until released, keyboard/gamepad is ignored;
  controller.set({}), controller.set(nil), or controller.set() to return control to keyboard/gamepad
controller.check(): prints buttons currently held

Emulator:
emulator.pause()  emulator.resume()  emulator.step_instruction()  emulator.step_frame()  emulator.reset()
emulator.screenshot()  emulator.start_gif()  emulator.stop_gif()  emulator.start_video()  emulator.stop_video()
emulator.save_state(slot)  emulator.load_state(slot)  emulator.delete_state(slot)
  - slot is a value from 1 to 12
emulator.elapsed_cycles(): GB/GBC reports t-cycles
emulator.pause_at_cycle(cycle)
emulator.pause_after_elapsed_cycles(elapsed_cycles)

Breakpoints:
debug.set_breakpoint(address, {pause = true})
debug.remove_breakpoint(address)
debug.clear_all_breakpoints()
debug.check_breakpoints()

Watchpoints (bus accesses):
debug.set_watchpoint(start_address, {end_address, pause = true, on = "rw", width = "byte", target = nil, accessor = "all"})
  - on: "read", "write", "rw", "change" (change = write of a value different from the last write)
  - width: "byte", "halfword", "word" (gameboy: byte only; halfword/word addresses are aligned for gba)
    a watchpoint only fires for the specified access width
  - target: on writes it is the incoming value being written; on reads it is the current value
    at the address being accessed; the watchpoint only fires for the specified target
  - end_address: if included the watchpoint becomes an inclusive range (start_address..=end_address)
  - accessor: "all", "cpu", "dma"; restricts hits to the hardware accessing memory
debug.remove_watchpoint(address, {end_address}): end_address is required to remove a range
debug.clear_all_watchpoints()
debug.check_watchpoints()

Scripts:
script.stop(): stops Lua scripts, hooks, and clears all watchpoints and breakpoints

Hooks:
function emulator.on_frame(): runs once per frame
function debug.on_breakpoint(address): runs when a breakpoint is hit
function debug.on_watchpoint(hit): runs once per watchpoint hit; hit is a table with
  address, value, pc, on, width, pause, cycle, accessor, domain, offset, pc_offset
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
    SaveState(usize),
    LoadState(usize),
    DeleteState(usize),
    StartVideo,
    StopVideo,
}

// is it incredibly lazy to create a new enum to compare strings with another nearly duplicate enum with a one variant difference
// in order to avoid a refactor? yes. but does it work and was it incredibly easy to do? also yes. will i refactor this later? nope
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Accessor {
    Cpu,
    Dma,
    All,
}

impl Accessor {
    pub fn to_string(self) -> &'static str {
        match self {
            Accessor::Cpu => "cpu",
            Accessor::Dma => "dma",
            Accessor::All => "all",
        }
    }

    pub fn to_enum(string: String) -> Result<Accessor, mlua::Error> {
        let lower = string.to_lowercase();
        let valid = ["all", "cpu", "dma"].join(", ");
        match lower.as_str() {
            "cpu" => Ok(Accessor::Cpu),
            "dma" => Ok(Accessor::Dma),
            "all" => Ok(Accessor::All),
            _ => Err(mlua::Error::runtime(format!(
                "'{string}' is invalid: (valid: {valid})"
            ))),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum MemoryAccessor {
    Cpu,
    Dma, // for organization but may do something wiht it later
    Lua,
}

impl MemoryAccessor {
    pub fn to_string(self) -> &'static str {
        match self {
            MemoryAccessor::Cpu => "cpu",
            MemoryAccessor::Dma => "dma",
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddressRange {
    Single,
    Range(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchpointRange {
    pub start_address: u32,
    pub end_address: Option<u32>,
}

pub fn find_watchpoint(
    address: u32,
    watchpoints: &HashMap<WatchpointRange, WatchpointArgs>,
) -> Option<WatchpointRange> {
    for range in watchpoints.keys() {
        let end_address = range.end_address.unwrap_or(range.start_address);

        if address >= range.start_address && address <= end_address {
            return Some(*range);
        }
    }

    None
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
                "invalid option for `on`: {string}: valid options are 'read', 'write', 'rw', and 'change'."
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
    pub accessor: Accessor,
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
    pub cycle: u64,
    pub accessor: MemoryAccessor,
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
    kwargs: &Option<Table>,
) -> Result<(WatchpointRange, WatchpointArgs), mlua::Error> {
    let (end_address, pause, on, width, target, accessor) = match kwargs {
        Some(table) => (
            table.get::<Option<u32>>("end_address")?,
            table.get::<Option<bool>>("pause")?,
            table.get::<Option<String>>("on")?,
            table.get::<Option<String>>("width")?,
            table.get::<Option<u32>>("target")?,
            table.get::<Option<String>>("accessor")?,
        ),
        None => (None, None, None, None, None, None),
    };

    let pause = pause.unwrap_or(true);
    let on = WatchpointType::from_string(on.unwrap_or_else(|| "rw".to_string()))?;
    let width = Width::from_string(width.unwrap_or_else(|| "byte".to_string()))?;
    let end_address = if let Some(end) = end_address {
        if end <= address {
            return Err(mlua::Error::runtime(format!(
                "`end_address`: {end} cannot be less than or equal to `start_address`: {address}"
            )));
        }

        Some(width.align(end))
    } else {
        None
    };
    let accessor = if let Some(string) = accessor {
        Accessor::to_enum(string)?
    } else {
        Accessor::All
    };

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
        accessor,
    };

    let watchpoint_range = WatchpointRange {
        start_address: width.align(address),
        end_address,
    };

    Ok((watchpoint_range, watchpoint_args))
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
    stop_scripts: bool,
    clear_console: bool,
    memory: Table,
    debug: Table,
    controller: Table,
    color: Table,
    emulator: Table,
    script: Table,
    cpu: Table,
    console: Table,
}

impl ScriptEngine {
    pub fn new() -> Self {
        //https://docs.rs/mlua/latest/mlua/struct.Thread.html
        let lua = Lua::new();

        let memory = lua.create_table().unwrap();
        let _ = lua.globals().set("memory", &memory);

        let debug = lua.create_table().unwrap();
        let _ = lua.globals().set("debug", &debug);

        let controller = lua.create_table().unwrap();
        let _ = lua.globals().set("controller", &controller);

        let color = lua.create_table().unwrap();
        let _ = lua.globals().set("color", &color);

        let emulator = lua.create_table().unwrap();
        let _ = lua.globals().set("emulator", &emulator);

        let script = lua.create_table().unwrap();
        let _ = lua.globals().set("script", &script);

        let cpu = lua.create_table().unwrap();
        let _ = lua.globals().set("cpu", &cpu);

        let console = lua.create_table().unwrap();
        let _ = lua.globals().set("console", &console);

        let _ = lua
            .load(
                r#"
            function emulator.step_instruction()
                emulator._step_instruction()
                coroutine.yield()
            end

            function emulator.step_frame()
                emulator._step_frame()
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
            stop_scripts: false,
            clear_console: false,
            memory,
            debug,
            controller,
            color,
            emulator,
            script,
            cpu,
            console,
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

    pub fn stop_scripts(&mut self) {
        self.step_completed();
        self.running = None;
        self.stop_scripts = true;
    }

    fn stop_all_scripts(&mut self, target: &mut dyn ScriptTarget) {
        if take(&mut self.stop_scripts) {
            self.running = None;
            self.pending.clear();
            self.step_done = false;
            let _ = self.emulator.set("on_frame", Value::Nil);
            let _ = self.debug.set("on_breakpoint", Value::Nil);
            let _ = self.debug.set("on_watchpoint", Value::Nil);
            target.clear_all_breakpoints();
            target.clear_all_watchpoints();
            target.take_breakpoint_hit();
            target.take_watchpoint_hits();

            self.output.push(
                "all running hooks, watchpoints, and breakpoints have been terminated".to_string(),
            );
        }
    }

    pub fn take_clear_request(&mut self) -> bool {
        take(&mut self.clear_console)
    }

    pub fn execute(
        &mut self,
        target: &mut dyn ScriptTarget,
        emulator_id: EmulatorId,
        frame_boundary: bool,
    ) {
        let Self {
            lua,
            pending,
            output,
            running,
            requests,
            step_done,
            stop_scripts,
            clear_console,
            memory,
            debug,
            controller,
            color,
            emulator,
            script,
            cpu,
            console,
        } = self;

        let target = RefCell::new(target);
        let output = RefCell::new(output);
        let requests = RefCell::new(requests);
        let clear_console = Cell::from_mut(clear_console);
        let stop_scripts = Cell::from_mut(stop_scripts);

        let result = lua.scope(|scope| {
            script.set("stop", scope.create_function(|_,  (): ()| {
                stop_scripts.replace(true);

                Ok(())
            })?)?;

            // always liked the name even though duplicate function
            let print = scope.create_function(|_, values: Variadic<Value>| {
                let line = values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<mlua::Result<Vec<String>>>()?
                    .join("\t");

                output.borrow_mut().push(line);

                Ok(())
            })?;
            lua.globals().set("print", &print)?;
            console.set("log", print)?;

            console.set("clear", scope.create_function(|_, (): ()| {
                clear_console.set(true);

                output.borrow_mut().clear();

                Ok(())
            })?)?;

            memory.set("read_u8", scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u8(address)))?)?;

            memory.set("read_u16", scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u16(address)))?)?;

            memory.set("read_u32", scope
                .create_function(|_, address: u32| Ok(target.borrow_mut().read_u32(address)))?)?;

             memory.set("write_u8", scope.create_function(|_, (address, value): (u32, u8)| {
                target.borrow_mut().write_u8(address, value);
                Ok(())
            })?)?;

            memory.set("write_u16", scope.create_function(|_, (address, value): (u32, u16)| {
                target.borrow_mut().write_u16(address, value);
                Ok(())
            })?)?;

            memory.set("write_u32", scope.create_function(|_, (address, value): (u32, u32)| {
                target.borrow_mut().write_u32(address, value);
                Ok(())
            })?)?;

            memory.set("domain_names", scope
                .create_function(|_, (): ()| Ok(target.borrow().memory_domain_names().to_vec()))?)?;

            memory.set("read_domain", scope.create_function(|_, (domain, offset): (String, usize)| {
                let result = target.borrow().read_domain(&domain, offset);
                result.map_err(|err| {
                    domain_error(&domain.to_lowercase(), offset, err, target.borrow().memory_domain_names())
                })
            })?)?;

            memory.set("write_domain",
                scope.create_function(|_, (domain, offset, value): (String, usize, u8)| {
                    let result = target.borrow_mut().write_domain(&domain, offset, value);
                    result.map_err(|err| {
                        domain_error(&domain.to_lowercase(), offset, err, target.borrow().memory_domain_names())
                    })
                })?)?;

            memory.set("address_to_domain", scope.create_function(|_, address: u32| {
                target.borrow().address_to_domain(address).ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "address {address:#010x} not mapped to any memory domain"
                    ))
                })
            })?)?;

            cpu.set("register_names", scope
                .create_function(|_, (): ()| Ok(target.borrow().cpu_register_names().to_vec()))?)?;

            cpu.set("read_register", scope.create_function(|_, name: String| {
                let name = name.to_lowercase();
                let result = target.borrow().read_cpu_register(name.clone());
                result.map_err(|err| cpu_error(&name, err, target.borrow().cpu_register_names()))
            })?)?;

            cpu.set("write_register", scope.create_function(|_, (name, value): (String, u32)| {
                let name = name.to_lowercase();
                let result = target.borrow_mut().write_cpu_register(name.clone(), value);
                result.map_err(|err| cpu_error(&name, err, target.borrow().cpu_register_names()))
            })?)?;

            color.set("to_rgb_hex", scope.create_function(|_, value: u32| {
                let [r, g, b] = target.borrow().to_rgb(value);
                Ok(format!("#{r:02x}{g:02x}{b:02x}"))
            })?)?;

            controller.set("names", scope.create_function(|_,  (): ()| {
                Ok(get_available_inputs(emulator_id))
            })?)?;

            controller.set("set", scope.create_function(|_, kwargs: Option<Table>| {
                let mask = map_inputs(emulator_id, kwargs);
                target.borrow_mut().set_scripted_inputs(mask);

                Ok(())
            })?)?;

            controller.set("check", scope.create_function(|_,  (): ()| {
                let message = match target.borrow().check_scripted_inputs() {
                    Some(key_mask) => format!("The following keys are set: {}", bool_to_keys(emulator_id, key_mask).join(", ")),
                    None => "No keys set".to_string()
                };

                output.borrow_mut().push(message);

                Ok(())
            })?)?;

            emulator.set("elapsed_cycles", scope.create_function(|_, (): ()| {
                        Ok(target.borrow().elapsed_cycles())
            })?)?;

            let set_emulator_controls =
                |name: &'static str, request: fn() -> ScriptRequest| {
                    let requests = &requests;
                    let function = scope.create_function(move |_, (): ()| {
                        requests.borrow_mut().push(request());
                        Ok(())
                    })?;

                    emulator.set(name, function)
                };

            set_emulator_controls("pause", || ScriptRequest::Pause)?;
            set_emulator_controls("resume", || ScriptRequest::Resume)?;
            set_emulator_controls("_step_instruction", || ScriptRequest::StepInstruction)?;
            set_emulator_controls("_step_frame", || ScriptRequest::StepFrame)?;
            set_emulator_controls("reset", || ScriptRequest::Reset)?;
            set_emulator_controls(
                "screenshot",
                || ScriptRequest::Screenshot,
            )?;
            set_emulator_controls(
                "start_gif",
                || ScriptRequest::StartGif,
            )?;
            set_emulator_controls(
                "stop_gif",
                || ScriptRequest::StopGif,
            )?;
            set_emulator_controls(
                "start_video",
                || ScriptRequest::StartVideo,
            )?;
            set_emulator_controls(
                "stop_video",
                || ScriptRequest::StopVideo,
            )?;

            let set_emulator_save_state = |name: &'static str, request: fn(usize) -> ScriptRequest| {
                let requests = &requests;
                let output = &output;
                let function = scope.create_function(move |_, slot: usize| {
                if  !(1..=SAVE_STATE_SLOTS).contains(&slot) {
                        output.borrow_mut().push(mlua::Error::runtime(format!("Only save states 1-{SAVE_STATE_SLOTS} can be used")).to_string());
                    } else {
                        requests.borrow_mut().push(request(slot));
                    }

                    Ok(())
                })?;

                emulator.set(name, function)
            };

            set_emulator_save_state("save_state", ScriptRequest::SaveState)?;
            set_emulator_save_state("load_state", ScriptRequest::LoadState)?;
            set_emulator_save_state("delete_state", ScriptRequest::DeleteState)?;

            emulator.set("pause_at_cycle", scope
                .create_function(|_, cycle: u64| {
                    target.borrow_mut().set_cycle_pause(cycle);

                    Ok(())
                })?)?;

            emulator.set("pause_after_elapsed_cycles",  scope
                .create_function(|_, elapsed_cycles: u64| {
                    let current_cycle = target.borrow().elapsed_cycles();
                    target.borrow_mut().set_elapsed_cycles_pause(elapsed_cycles + current_cycle);

                    Ok(())
                })?)?;

            let strings =  target.borrow_mut().cycle_paused_text();
            for string in strings {
                output.borrow_mut().push(string);
            }

            if frame_boundary && let Ok(hook) = emulator.get::<Function>("on_frame") {
                time_limit(lua, Duration::from_millis(5));
                if let Err(err) = hook.call::<()>(()) {
                    output.borrow_mut().push(format!("{err}"));
                    let _ = emulator.set("on_frame", Value::Nil);
                }
            }

            debug.set("set_breakpoint",
                scope.create_function(|_, (address, kwargs): (u32, Option<Table>)| {
                    check_address(emulator_id, address)?;

                    let pause = match kwargs {
                        Some(table) => table.get::<Option<bool>>("pause")?.unwrap_or(true),
                        None => true,
                    };
                    let message = if target.borrow_mut().set_breakpoint(address, pause) {
                        format!("breakpoint set at {address:08X}")
                    } else {
                        format!("breakpoint at {address:08X} was updated")
                    };

                    output.borrow_mut().push(message);
                    Ok(())
                })?)?;

            debug.set("remove_breakpoint", scope.create_function(|_, address: u32| {
                target.borrow_mut().remove_breakpoint(address);
                output
                    .borrow_mut()
                    .push(format!("breakpoint at {address:08X} removed"));
                Ok(())
            })?)?;

            debug.set("clear_all_breakpoints", scope.create_function(|_, (): ()| {
                target.borrow_mut().clear_all_breakpoints();
                output
                    .borrow_mut()
                    .push("all breakpoints cleared".to_string());
                Ok(())
            })?)?;

            debug.set("check_breakpoints", scope.create_function(|_, (): ()| {
                let breakpoints = target.borrow_mut().check_breakpoints();
                if !breakpoints.is_empty() {
                    for (address, state) in breakpoints.iter() {
                        let action = if *state == EmulatorState::Paused {
                            "pause"
                        } else {
                            "run"
                        };

                        output.borrow_mut().push(format!(
                            "breakpoint at {:08X}, action on breakpoint: {action}",
                            *address
                        ))
                    }
                } else {
                    output.borrow_mut().push("no breakpoints found".to_string())
                }
                Ok(())
            })?)?;

            debug.set("set_watchpoint",
                scope.create_function(|_, (address, kwargs): (u32, Option<Table>)| {
                    check_address(emulator_id, address)?;
                    let (watchpoint_range, watchpoint_args) =
                        parse_watchpoint_args(emulator_id, address, &kwargs)?;
                    let in_hashmap = target.borrow_mut().set_watchpoint(watchpoint_range, watchpoint_args);
                    let message = if in_hashmap {
                        if let Some(end_address) = watchpoint_range.end_address {
                            format!("watchpoint address range {address:08X}..={end_address:08X} was updated")
                        } else {
                            format!("watchpoint at {address:08X} was updated")
                        }
                    } else {
                        if let Some(end_address) = watchpoint_range.end_address {
                            format!("watchpoint set at address range {address:08X}..={end_address:08X}")
                        } else {
                            format!("watchpoint set at address {address:08X}")
                        }
                    };

                    output.borrow_mut().push(message);
                    Ok(())
                })?)?;

            debug.set("remove_watchpoint", scope.create_function(|_, (address, kwargs):  (u32, Option<Table>)| {
                let watchpoint_range = WatchpointRange {
                    start_address: address,
                    end_address: if let Some(table) = kwargs && let Ok(some_option) = table.get::<Option<u32>>("end_address") && let Some(end_address) = some_option {
                        Some(end_address)
                    } else {
                        None
                    }
                };

                target.borrow_mut().remove_watchpoint(watchpoint_range);

                if let Some(end_address) = watchpoint_range.end_address {
                    output
                        .borrow_mut()
                        .push(format!("watchpoint range at {address:08X}..={end_address:08X} removed"));
                } else {
                    output
                        .borrow_mut()
                        .push(format!("watchpoint at {address:08X} removed"));
                }

                Ok(())
            })?)?;

            debug.set("clear_all_watchpoints", scope.create_function(|_, (): ()| {
                target.borrow_mut().clear_all_watchpoints();
                output
                    .borrow_mut()
                    .push("all watchpoints cleared".to_string());
                Ok(())
            })?)?;

            debug.set("check_watchpoints", scope.create_function(|_, (): ()| {
                let watchpoints = target.borrow_mut().check_watchpoints();
                if !watchpoints.is_empty() {
                    for (watchpoint_range, watchpoint_args) in watchpoints.iter() {
                        let start_address = watchpoint_range.start_address;
                        let range = if let Some(end_address) = watchpoint_range.end_address {
                            format!("watchpoint range at {start_address:08X}..={end_address:08X}")
                        } else {
                            format!("watchpoint at {start_address:08X}")
                        };
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
                            "{range} on {on} for {width} access{}, action on watchpoint: {action}, accessor: {}",
                            target_message, watchpoint_args.accessor.to_string()
                        ))
                    }
                } else {
                    output.borrow_mut().push("no watchpoints found".to_string())
                }
                Ok(())
            })?)?;

            // yeah default behavior when no hook is less annoying
            let hit = target.borrow_mut().take_breakpoint_hit();
            if let Some(address) = hit {
                if let Ok(hook) = debug.get::<Function>("on_breakpoint") {
                    time_limit(lua, Duration::from_millis(5));
                    if let Err(err) = hook.call::<()>(address) {
                        output.borrow_mut().push(format!("{err}"));
                        let _ = debug.set("on_breakpoint", Value::Nil);
                    }
                } else {
                    output
                    .borrow_mut()
                    .push(format!("breakpoint at {address:08x} reached"));
                }
            }

            let hits = target.borrow_mut().take_watchpoint_hits();
            if !hits.is_empty() {
                let hook = debug.get::<Function>("on_watchpoint").ok();
                for hit in hits {
                    if hook.is_none() {
                        output.borrow_mut().push(format!(
                            "watchpoint at {:08x} hit: {} {} value={:x} pc={:08x}, accessor={}",
                            hit.address,
                            hit.on.to_string(),
                            hit.width.to_string(),
                            hit.value,
                            hit.pc,
                            hit.accessor.to_string()
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
                    table.set("cycle", hit.cycle)?;
                    table.set("accessor", hit.accessor.to_string())?;

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
                        let _ = debug.set("on_watchpoint", Value::Nil);
                        break;
                    }
                }
            }

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

                if stop_scripts.get() {
                    return Ok(());
                }
            };

            Ok(())
        });

        if let Err(err) = result {
            output
                .borrow_mut()
                .push(format!("script engine error: {err}"));
        }

        self.stop_all_scripts(target.into_inner());
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
