use std::sync::Arc;

use hyper::{Body, Request, Response};
use rlua::{FromLuaMulti, Function, Lua, MultiValue, ToLuaMulti};
use tokio::sync::{mpsc, oneshot, Mutex};

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

        request.into_request()
    }

    pub fn call_on_http_response(&self, res: ProxyResponse) -> anyhow::Result<Response<Body>> {
        let response = self.call_lua_function::<_, ProxyResponse>("on_http_response", res)?;
        response.into_response()
    }
}

#[derive(Debug)]
enum ProxyData {
    Request {
        arg: ProxyRequest,
        responder: oneshot::Sender<Request<Body>>,
    },
    Response {
        arg: ProxyResponse,
        responder: oneshot::Sender<Response<Body>>,
    },
}

#[derive(Clone)]
pub struct Messenger {
    sender: mpsc::UnboundedSender<ProxyData>,
}

impl Messenger {
    pub async fn call_on_http_request(&self, req: ProxyRequest) -> anyhow::Result<Request<Body>> {
        let (otx, orx) = oneshot::channel();

        let request = ProxyData::Request {
            arg: req,
            responder: otx,
        };

        self.sender.send(request)?;

        Ok(orx.await?)
    }

    pub async fn call_on_http_response(
        &self,
        res: ProxyResponse,
    ) -> anyhow::Result<Response<Body>> {
        let (otx, orx) = oneshot::channel();

        let request = ProxyData::Response {
            arg: res,
            responder: otx,
        };

        self.sender.send(request)?;

        Ok(orx.await?)
    }
}

pub struct LuaPool {
    _workers: Vec<Worker>,
}

impl LuaPool {
    pub fn build(size: usize, lua_code: String) -> anyhow::Result<(Self, Messenger)> {
        assert!(size > 0);

        let (sender, receiver) = mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            workers.push(Worker::build(receiver.clone(), lua_code.to_string())?)
        }

        Ok((LuaPool { _workers: workers }, Messenger { sender }))
    }
}

struct Worker {
    _handle: Option<tokio::task::JoinHandle<Result<(), anyhow::Error>>>,
}

impl Worker {
    fn build(
        reciver: Arc<Mutex<mpsc::UnboundedReceiver<ProxyData>>>,
        lua_code: String,
    ) -> anyhow::Result<Worker> {
        let thread = tokio::spawn(async move {
            let lua_engine = LuaEngine::new();
            lua_engine.load(lua_code.as_str())?;

            while let Some(msg) = reciver.lock().await.recv().await {
                match msg {
                    ProxyData::Request { arg, responder } => {
                        let req = lua_engine.call_on_http_request(arg)?;
                        let _ = responder.send(req);
                    }
                    ProxyData::Response { arg, responder } => {
                        let res = lua_engine.call_on_http_response(arg)?;
                        let _ = responder.send(res);
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        });

        Ok(Worker {
            _handle: Some(thread),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;

    #[tokio::test]
    async fn test_lua_engine() {
        let lua_code = read_to_string("./filter.lua").expect("read file");
        let capacity = 12;

        let (_, msgr) = LuaPool::build(capacity, lua_code).expect("LuaPool build failed");

        let mut handles = Vec::with_capacity(capacity);

        for _ in 0..capacity {
            let msgr = msgr.clone();
            handles.push(tokio::spawn(async move {
                let request = ProxyRequest::new("http://google.com", "GET", "Hello");
                let _request = msgr.call_on_http_request(request).await;
            }))
        }
        {
            let msgr = msgr.clone();
            handles.push(tokio::spawn(async move {
                let request = ProxyRequest::new("http://google.com", "GET", "load");
                let _request = msgr.call_on_http_request(request).await;
            }));
        }

        let response = ProxyResponse::new(200, "bye");
        let _response = msgr.call_on_http_response(response).await;

        for h in handles {
            let _ = h.await;
        }
    }
}
