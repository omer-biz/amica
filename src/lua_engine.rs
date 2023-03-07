use hyper::{Body, Request, Response};
use rlua::{FromLuaMulti, Function, Lua, MultiValue, ToLuaMulti};

use crate::proxy::{ProxyRequest, ProxyResponse};

pub(crate) struct LuaEngine {
    lua_vm: Lua,
}

impl LuaEngine {
    pub(crate) fn new() -> Self {
        LuaEngine { lua_vm: Lua::new() }
    }

    pub(crate) fn load(&self, lua_code: &str) -> rlua::Result<()> {
        self.lua_vm.context(|lua_context| {
            lua_context.load(lua_code).eval::<MultiValue>()?;
            Ok(())
        })?;

        Ok(())
    }

    fn call_lua_function<A, R>(&self, function: &str, args: A) -> rlua::Result<R>
    where
        A: for<'lua> ToLuaMulti<'lua>,
        R: for<'lua> FromLuaMulti<'lua>,
    {
        self.lua_vm.context(move |lua_context| {
            let lua_function = lua_context.globals().get::<_, Function>(function)?;
            let lua_result = lua_function.call::<A, R>(args)?;

            Ok(lua_result)
        })
    }

    pub(crate) fn call_on_http_request(&self, req: ProxyRequest) -> rlua::Result<Request<Body>> {
        let request = self.call_lua_function::<_, ProxyRequest>("on_http_request", req)?;
        Ok(request.into())
    }

    pub(crate) fn call_on_http_response(&self, res: ProxyResponse) -> rlua::Result<Response<Body>> {
        let response = self.call_lua_function::<_, ProxyResponse>("on_http_response", res)?;
        Ok(response.into())
    }
}
