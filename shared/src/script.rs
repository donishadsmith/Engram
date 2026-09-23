use crate::{EmulatorId, ScriptTarget};
use mlua::{Function, HookTriggers, Lua, Table, Value, Variadic, VmState};
use std::{
    cell::{Cell, RefCell},
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
set_breakpoint(address, {pause: bool})
remove_breakpoint(address)
clear_all_breakpoints()

Emulator controls:
pause()  resume()  step()  reset()
screenshot()  start_gif()  stop_gif()

Hooks:
function on_frame(): runs once per frame
function on_breakpoint(address): runs when a breakpoint is hit
"#;

pub enum DomainError {
    UnknownDomain,
    OutOfRange { size: usize },
}

pub enum CpuError {
    UnknownRegister,
    ReadOnly,
}

pub enum ScriptRequest {
    Pause,
    Screenshot,
    StartGif,
    StopGif,
    Reset,
    Step,
    Resume,
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

pub struct ScriptEngine {
    lua: Lua,
    pending: VecDeque<String>,
    output: Vec<String>,
    requests: Vec<ScriptRequest>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
            pending: VecDeque::new(),
            output: Vec::new(),
            requests: Vec::new(),
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

    pub fn execute(
        &mut self,
        target: &mut dyn ScriptTarget,
        _emulator_id: EmulatorId,
        frame_boundary: bool,
    ) {
        let Self {
            lua,
            pending,
            output,
            requests,
        } = self;

        let target = RefCell::new(target);
        let output = RefCell::new(output);
        let requests = RefCell::new(requests);
        let printed = Cell::new(0u32);

        let result = lua.scope(|scope| {
            let print = scope.create_function(|_, vals: Variadic<Value>| {
                printed.set(printed.get() + 1);
                if printed.get() > 1000 {
                    return Err(mlua::Error::runtime(
                        "script terminated due to too much output",
                    ));
                }

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
                    domain_error(&domain, offset, err, target.borrow().memory_domain_names())
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
                        domain_error(&domain, offset, err, target.borrow().memory_domain_names())
                    })
                })?;
            lua.globals().set("write_domain", write_domain)?;

            let cpu_register_names = scope
                .create_function(|_, (): ()| Ok(target.borrow().cpu_register_names().to_vec()))?;
            lua.globals()
                .set("cpu_register_names", cpu_register_names)?;

            let read_cpu_register = scope.create_function(|_, name: String| {
                let result = target.borrow().read_cpu_register(name.clone());
                result.map_err(|err| cpu_error(&name, err, target.borrow().cpu_register_names()))
            })?;
            lua.globals().set("read_cpu_register", read_cpu_register)?;

            let write_cpu_register = scope.create_function(|_, (name, value): (String, u32)| {
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

            let set_breakpoint =
                scope.create_function(|_, (address, kwargs): (u32, Option<Table>)| {
                    let pause = match kwargs {
                        Some(table) => table.get::<Option<bool>>("pause")?.unwrap_or(true),
                        None => true,
                    };
                    let message = if target.borrow_mut().set_breakpoint(address, pause) {
                        format!("breakpoint set at {address:08x}")
                    } else {
                        format!("breakpoint at {address:08x} already exists")
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
            control("step", || ScriptRequest::Step, "")?;
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

            while let Some(code) = pending.pop_front() {
                time_limit(lua, Duration::from_millis(100));
                match lua.load(&code).eval::<mlua::MultiValue>() {
                    Ok(values) if !values.is_empty() => output.borrow_mut().push(
                        values
                            .iter()
                            .map(display_value)
                            .collect::<Vec<_>>()
                            .join("\t"),
                    ),
                    Ok(_) => {}
                    Err(e) => output.borrow_mut().push(format!("{e}")),
                }
            }

            if frame_boundary && let Ok(hook) = lua.globals().get::<Function>("on_frame") {
                time_limit(lua, Duration::from_millis(5));
                if let Err(e) = hook.call::<()>(()) {
                    output.borrow_mut().push(format!("{e}"));
                    let _ = lua.globals().set("on_frame", Value::Nil);
                }
            }

            let hit = target.borrow_mut().take_breakpoint_hit();
            if let Some(address) = hit {
                output
                    .borrow_mut()
                    .push(format!("breakpoint at {address:08x} reached"));

                if let Ok(hook) = lua.globals().get::<Function>("on_breakpoint") {
                    time_limit(lua, Duration::from_millis(5));
                    if let Err(e) = hook.call::<()>(address) {
                        output.borrow_mut().push(format!("{e}"));
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
