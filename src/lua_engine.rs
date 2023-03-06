use rlua::{Function, Lua, MultiValue};

use crate::proxy::ProxyRequest;

pub(crate) struct LuaEngine {
    lua_vm: Lua,
}

impl LuaEngine {
    pub(crate) fn new() -> Self {
        LuaEngine { lua_vm: Lua::new() }
    }

    pub(crate) fn load(&self, lua_code: &str) -> Result<(), ()> {
        self.lua_vm.context(|lua_context| {
            let a = lua_context.load(lua_code).eval::<MultiValue>();
            if a.is_err() {
                return Err(());
            }
            Ok(())
        })?;

        Ok(())
    }

    pub(crate) fn call_on_http_request(&self, req: ProxyRequest) -> Result<ProxyRequest, ()> {
        self.lua_vm.context(|lua_context| {
            let on_http_request = lua_context.globals().get::<_, Function>("on_http_request");
            if on_http_request.is_err() {
                return Err(());
            }
            let on_http_request = on_http_request.unwrap();

            let request = on_http_request.call::<_, ProxyRequest>(req);
            if request.is_err() {
                return Err(());
            }
            Ok(request.unwrap())
        })
    }
}
