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

    pub(crate) fn load(&self, lua_code: &str) -> Result<(), ()> {
        self.lua_vm.context(|lua_context| {
            let a = lua_context.load(lua_code).eval::<MultiValue>();
            if a.is_err() {
                println!("{}", a.unwrap_err());
                return Err(());
            }
            Ok(())
        })?;

        Ok(())
    }

    fn call_lua_function<A, R>(&self, function: &str, args: A) -> Result<R, ()>
    where
        A: for<'lua> ToLuaMulti<'lua>,
        R: for<'lua> FromLuaMulti<'lua>,
    {
        self.lua_vm.context(move |lua_context| {
            let lua_function = lua_context.globals().get::<_, Function>(function);
            if lua_function.is_err() {
                eprintln!("{}", lua_function.unwrap_err());
                return Err(());
            }
            let lua_function = lua_function.unwrap();

            let lua_result = lua_function.call::<A, R>(args);

            if lua_result.is_err() {
                return Err(());
            }

            Ok(lua_result.unwrap())
        })
    }

    pub(crate) fn call_on_http_request(&self, req: ProxyRequest) -> Result<Request<Body>, ()> {
        let r = self.call_lua_function::<_, ProxyRequest>("on_http_request", req);

        Ok(r.unwrap().into())
    }

    pub(crate) fn call_on_http_response(&self, res: ProxyResponse) -> Result<Response<Body>, ()> {
        let r = self.call_lua_function::<_, ProxyResponse>("on_http_response", res);

        Ok(r.unwrap().into())
    }
}
