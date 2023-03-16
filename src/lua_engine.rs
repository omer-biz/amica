use hyper::{Body, Request, Response};
use rlua::{FromLuaMulti, Function, Lua, MultiValue, ToLuaMulti};

use crate::intermediate_proxy_data::{ProxyRequest, ProxyResponse};

pub struct LuaEngine {
    lua_vm: Lua,
}

impl LuaEngine {
    pub fn new() -> Self {
        LuaEngine { lua_vm: Lua::new() }
    }

    pub fn load(&self, lua_code: &str) -> rlua::Result<()> {
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

    pub fn call_on_http_request(&self, req: ProxyRequest) -> anyhow::Result<Request<Body>> {
        let request = self.call_lua_function::<_, ProxyRequest>("on_http_request", req)?;

        request.to_request()
    }

    pub fn call_on_http_response(&self, res: ProxyResponse) -> anyhow::Result<Response<Body>> {
        let response = self.call_lua_function::<_, ProxyResponse>("on_http_response", res)?;
        response.to_response()
    }
}

// will work on this some other time
// pub struct LuaPool {
//     pool: Vec<Option<LuaEngine>>,
//     receiver: mpsc::UnboundedReceiver<Message>,
//     lua_code: String,
// }
//
// #[derive(Clone)]
// pub struct Messenger {
//     sender: mpsc::UnboundedSender<Message>,
// }
//
// impl Messenger {
//     pub async fn call_func(&self, function: &str, args: (u32, u32)) -> anyhow::Result<String> {
//         let (otx, orx) = oneshot::channel();
//         let msg = Message {
//             function: function.to_string(),
//             args,
//             responder: otx,
//         };
//
//         self.sender.send(msg)?;
//
//         Ok(orx.await?)
//     }
// }
//
// #[derive(Debug)]
// struct Message {
//     args: (u32, u32),
//     function: String,
//     responder: oneshot::Sender<String>,
// }
//
// impl LuaPool {
//     pub fn init(size: usize, code: &str) -> anyhow::Result<(Self, Messenger)> {
//         let (mtx, mrx) = mpsc::unbounded_channel();
//
//         let msgr = Messenger { sender: mtx };
//
//         let mut pool = Vec::with_capacity(size);
//
//         for _ in 0..size {
//             let engine = LuaEngine::new();
//             engine.load(code)?;
//
//             pool.push(Some(engine))
//         }
//
//         Ok((
//             Self {
//                 lua_code: code.to_string(),
//                 pool,
//                 receiver: mrx,
//             },
//             msgr,
//         ))
//     }
//
//     pub fn start(mut self) {
//         tokio::spawn(async move {
//             while let Some(msg) = self.receiver.recv().await {
//                 let Message {
//                     args,
//                     function,
//                     responder,
//                 } = msg;
//
//                 // println!("before: {:#?}\n\n", self.pool);
//
//                 let engine = self
//                     .pool
//                     .iter_mut()
//                     .find(|eng| eng.is_some())
//                     .unwrap()
//                     .take()
//                     .unwrap();
//
//                 // println!("after: {:#?}\n\n", self.pool);
//
//                 tokio::spawn(async move {
//                     let res = engine
//                         .call_lua_function::<_, String>(function.as_str(), args)
//                         .unwrap();
//                     responder.send(res).unwrap();
//                 });
//
//                 for eng in self.pool.iter_mut() {
//                     if eng.is_none() {
//                         let lua_eng = LuaEngine::new();
//                         lua_eng.load(self.lua_code.as_str()).unwrap();
//                         let _ = eng.insert(lua_eng);
//                         break;
//                     }
//                 }
//             }
//         });
//     }
// }
