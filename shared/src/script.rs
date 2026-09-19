use crate::ScriptTarget;
use mlua::{Function, Lua, Value, Variadic};
use std::{cell::RefCell, collections::VecDeque, mem::take};

const HELP: &str = r#"
Read memory address:
- read_u8(address)
- read_u16(address)
- read_u32(address)

Write memory address:
- write_u8(address, value)
- write_u16(address, value)
- write_u32(address, value)

Execute every frame:
function on_frame()
    ...
end
"#;

pub struct ScriptEngine {
    lua: Lua,
    pending: VecDeque<String>,
    output: Vec<String>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
            pending: VecDeque::with_capacity(100),
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

    pub fn execute(&mut self, target: &mut dyn ScriptTarget) {
        let Self {
            lua,
            pending,
            output,
        } = self;

        let mut errors: Vec<String> = Vec::new();
        let target = RefCell::new(target);
        let output = RefCell::new(output);

        let result = lua.scope(|scope| {
            let print = scope.create_function_mut(|_, vals: Variadic<Value>| {
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

            let help = scope.create_function(|_, ()| {
                output.borrow_mut().push(HELP.to_string());

                Ok(())
            })?;
            lua.globals().set("help", help)?;

            while let Some(code) = pending.pop_front() {
                if let Err(e) = lua.load(&code).exec() {
                    errors.push(format!("Error: {e}"));
                }
            }

            if let Ok(execute_every_frame) = lua.globals().get::<Function>("on_frame") {
                if let Err(e) = execute_every_frame.call::<()>(()) {
                    errors.push(format!("Error: {e}"));
                    let _ = lua.globals().set("on_frame", Value::Nil);
                }
            }

            Ok(())
        });

        if let Err(e) = result {
            errors.push(format!("Script Engine Error: {e}"));
        }

        output.borrow_mut().extend(errors);
    }
}
