use std::{path::PathBuf, sync::Arc};

use hyper::{Body, Request, Response};
use rlua::{FromLuaMulti, Function, Lua, MultiValue, ToLuaMulti};
use tokio::{
    fs::{self, read_to_string},
    sync::{mpsc, oneshot, Mutex},
};

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
    pub fn build(size: usize, lua_code: PathBuf) -> anyhow::Result<(Self, Messenger)> {
        assert!(size > 0);

        let (sender, receiver) = mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            workers.push(Worker::build(receiver.clone(), lua_code.clone())?)
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
        lua_script_path: PathBuf,
    ) -> anyhow::Result<Worker> {
        let thread = tokio::spawn(async move {
            let mut old_tstamp = fs::metadata(&lua_script_path).await?.accessed()?;

            let buf = read_to_string(&lua_script_path).await?;

            let lua_engine = LuaEngine::new();
            lua_engine.load(buf.as_str())?;

            while let Some(msg) = reciver.lock().await.recv().await {
                let new_tstamp = fs::metadata(&lua_script_path).await?.accessed()?;

                if new_tstamp > old_tstamp {
                    println!("New time stamp: {:?}, old: {:?}", new_tstamp, old_tstamp);
                    let buf = read_to_string(&lua_script_path).await?;
                    lua_engine.load(&buf)?;

                    old_tstamp = new_tstamp;
                }

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

    use hyper::body;

    use super::*;

    #[tokio::test]
    async fn test_lua_engine() {
        let capacity = 12;

        let (_, msgr) =
            LuaPool::build(capacity, PathBuf::from("./filter.lua")).expect("LuaPool build failed");

        let mut handles = Vec::with_capacity(capacity);

        for _ in 0..capacity {
            let msgr = msgr.clone();
            handles.push(tokio::spawn(async move {
                let request = ProxyRequest::_new("http://google.com", "GET", "Hello");
                let _request = msgr.call_on_http_request(request).await;
            }))
        }
        {
            let msgr = msgr.clone();
            handles.push(tokio::spawn(async move {
                let request = ProxyRequest::_new("http://google.com", "GET", "load");
                let _request = msgr.call_on_http_request(request).await;
            }));
        }

        let response = ProxyResponse::_new(200, "bye");
        let _response = msgr.call_on_http_response(response).await;

        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn test_messenger() -> Result<(), anyhow::Error> {
        let lua_code: PathBuf = "lua_test/messenger.lua".into();

        let (_, messenger) = LuaPool::build(10, lua_code)?;

        let mut res = ProxyResponse::_new(400, "Hello, World");
        res._with_header("header1", "value1");
        res._with_header("header2", "value2");
        res._with_header("header3", "value3");

        let mut expected_res = hyper::Response::builder()
            .status(401)
            .header("header1", "changed_value1")
            .header("header2", "changed_value2")
            .header("header3", "changed_value3")
            .header("new_header", "new_header")
            .header("content-length", "15")
            .body(Body::from("Good Bye, World"))?;

        let mut actual_res = messenger.call_on_http_response(res).await?;

        assert_response_eq(&actual_res, &expected_res);
        assert_body_eq(actual_res.body_mut(), expected_res.body_mut()).await?;

        let mut req = ProxyRequest::_new("http://example.com", "GET", "Hello, World");
        req._with_header("header1", "value1");
        req._with_header("header2", "value2");
        req._with_header("header3", "value3");

        let mut expected_req = hyper::Request::builder()
            .method("POST")
            .uri("http://www.example.com")
            .header("header1", "changed_value1")
            .header("header2", "changed_value2")
            .header("header3", "changed_value3")
            .header("new_header", "new_header")
            .header("content-length", "15")
            .body(Body::from("Good Bye, World"))?;

        let mut actual_req = messenger.call_on_http_request(req).await?;

        assert_requests_eq(&expected_req, &actual_req);
        assert_body_eq(actual_req.body_mut(), expected_req.body_mut()).await?;

        Ok(())
    }

    fn assert_requests_eq(req1: &Request<Body>, req2: &Request<Body>) {
        assert_eq!(req1.method(), req2.method());
        assert_eq!(req1.uri(), req2.uri());
        assert_eq!(req1.headers(), req2.headers());
    }

    async fn assert_body_eq(body1: &mut Body, body2: &mut Body) -> Result<(), anyhow::Error> {
        Ok(assert_eq!(
            body::to_bytes(body1).await?,
            body::to_bytes(body2).await?
        ))
    }

    fn assert_response_eq(res1: &Response<Body>, res2: &Response<Body>) {
        assert_eq!(res1.headers(), res2.headers());
        assert_eq!(res1.status(), res2.status());
    }
}
