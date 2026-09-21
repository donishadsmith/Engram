use crate::{EmulatorId, ScriptTarget};
use mlua::{Function, HookTriggers, Lua, Value, Variadic, VmState};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    mem::take,
    time::{Duration, Instant},
};

// https://github.com/ioncodes/gecko/blob/master/crates/scripting/src/lib.rs

const HELP: [(EmulatorId, &str); 1] = [(
    EmulatorId::Gba,
    r#"
Read memory address:
- read_u8(address)
- read_u16(address)
- read_u32(address)

Read CPU register:
- read_cpu_register(index)

Write memory address:
- write_u8(address, value)
- write_u16(address, value)
- write_u32(address, value)

Get rgb:
- to_rgb(read_u16(address))
- to_rgb_hex(read_u16(address))

Execute every frame:
function on_frame()
    ...
end
"#,
)];

pub struct ScriptEngine {
    lua: Lua,
    pending: VecDeque<String>,
    output: Vec<String>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
            pending: VecDeque::new(),
            output: Vec::new(),
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

    pub fn execute(
        &mut self,
        target: &mut dyn ScriptTarget,
        mut emulator_id: EmulatorId,
        frame_boundary: bool,
    ) {
        if emulator_id == EmulatorId::Gb {
            emulator_id = EmulatorId::Gba;
        }

        let help_vec: Vec<(EmulatorId, &str)> = HELP
            .iter()
            .copied()
            .filter(|x| x.0 == emulator_id)
            .collect();
        let help_string = Cell::new(help_vec[0].1.to_string());

        let Self {
            lua,
            pending,
            output,
        } = self;

        let target = RefCell::new(target);
        let output = RefCell::new(output);
        let printed = Cell::new(0u32);

        let _ = lua.scope(|scope| {
            let print = scope.create_function(|_, vals: Variadic<Value>| {
                printed.set(printed.get() + 1);
                if printed.get() > 1000 {
                    return Err(mlua::Error::runtime(
                        "script terminated due to too much output",
                    ));
                }

                let line = vals
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<mlua::Result<Vec<String>>>()?
                    .join("\t");

                output.borrow_mut().push(line);

                Ok(())
            })?;

            lua.globals().set("print", print)?;

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

            let read_cpu_register = scope.create_function(|_, register_name: String| {
                target
                    .borrow_mut()
                    .read_cpu_register(register_name.clone())
                    .ok_or_else(|| {
                        mlua::Error::runtime(format!("invalid CPU register: {register_name}"))
                    })
            })?;
            lua.globals().set("read_cpu_register", read_cpu_register)?;

            let to_rgb = scope.create_function(|_, value: u32| {
                let [r, g, b] = target.borrow_mut().to_rgb(value);

                Ok(format!("({r}, {g}, {b})"))
            })?;
            lua.globals().set("to_rgb", to_rgb)?;

            let to_rgb_hex = scope.create_function(|_, value: u32| {
                let [r, g, b] = target.borrow_mut().to_rgb(value);

                Ok(format!("#{r:02x}{g:02x}{b:02x}"))
            })?;
            lua.globals().set("to_rgb_hex", to_rgb_hex)?;

            let help = scope.create_function(|_, ()| {
                output.borrow_mut().push(help_string.take());

                Ok(())
            })?;
            lua.globals().set("help", help)?;

            while let Some(code) = pending.pop_front() {
                time_limit(lua, Duration::from_millis(100));
                if let Err(e) = lua.load(&code).exec() {
                    output.borrow_mut().push(format!("{e}"));
                }
            }

            if let Ok(hook) = lua.globals().get::<Function>("on_frame")
                && frame_boundary
            {
                time_limit(lua, Duration::from_millis(5));
                if let Err(e) = hook.call::<()>(()) {
                    output.borrow_mut().push(format!("{e}"));
                    let _ = lua.globals().set("on_frame", Value::Nil);
                }
            }

            Ok(())
        });
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
