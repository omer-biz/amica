use std::collections::HashMap;

use hyper::{body::to_bytes, server::conn::Http, service::service_fn, Body, Request, Response};
use rlua::{Function, MultiValue, UserData};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub async fn handle_client(mut client: TcpStream) {
    let mut buf = [0; 512];
    client.peek(&mut buf).await.unwrap();

    if buf.starts_with(&[67, 79, 78, 78, 69, 67, 84]) {
        let nbytes = client.read(&mut buf).await.unwrap();

        let head = String::from_utf8_lossy(&buf[..nbytes]);
        let host = head.split_whitespace().nth(1).unwrap();

        let server = TcpStream::connect(host).await.unwrap();

        client.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();

        bidi_read_write(server, client).await;
    } else {
        let _ = Http::new()
            .serve_connection(client, service_fn(handle_http_request))
            .await;
    }
}

async fn bidi_read_write(mut stream_one: TcpStream, mut stream_two: TcpStream) {
    let (mut stream_one_rx, mut stream_one_tx) = stream_one.split();
    let (mut stream_two_rx, mut stream_two_tx) = stream_two.split();

    let mut server_buf = [0; 4096];
    let mut client_buf = [0; 4096];
    loop {
        tokio::select! {
            Ok(n) = stream_one_rx.read(&mut server_buf) => {
                if n == 0 {
                    break;
                }
                stream_two_tx.write_all(&server_buf[..n]).await.unwrap();
            },
            Ok(n) = stream_two_rx.read(&mut client_buf) => {
                if n == 0 {
                    break;
                }
                stream_one_tx.write_all(&client_buf[..n]).await.unwrap();
            }
        }
    }
}

#[derive(Clone)]
struct ProxyRequest {
    uri: String,
    method: String,
    headers: HashMap<String, String>,
    body: String,
}

impl ProxyRequest {
    pub async fn new(request: Request<Body>) -> Self {
        // request.version();
        let (parts, body) = request.into_parts();
        let headers: HashMap<String, String> = parts
            .headers
            .iter()
            .map(|header| (header.0.to_string(), header.1.to_str().unwrap().to_string()))
            .collect();
        let body = &to_bytes(body).await.unwrap();
        let body = String::from_utf8_lossy(body);

        ProxyRequest {
            uri: parts.uri.to_string(),
            method: parts.method.to_string(),
            body: body.to_string(),
            headers,
        }
    }
}

impl Into<Request<Body>> for ProxyRequest {
    fn into(self) -> Request<Body> {
        let mut request = Request::builder()
            .method(self.method.as_str())
            .uri(self.uri.as_str());

        for (key, value) in self.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        request.body(Body::from(self.body)).unwrap()
    }
}

impl UserData for ProxyRequest {}

async fn handle_http_request(request: Request<Body>) -> Result<Response<Body>, String> {
    let lua_req_filter = r#"
    function on_http_request(req)
        print("request: ", req)

        return req
    end
    "#;

    let lua = rlua::Lua::new();
    let mut proxy_request = ProxyRequest::new(request).await;

    let request: Request<Body> = lua.context(|lua_context| {
        let globals = lua_context.globals();
        let _ = lua_context
            .load(lua_req_filter)
            .eval::<MultiValue>()
            .unwrap();

        let on_http_request: Function = globals.get("on_http_request").unwrap();

        proxy_request = on_http_request
            .call::<_, ProxyRequest>(proxy_request)
            .unwrap();
        proxy_request.into()
    });

    let hyper_client = hyper::Client::new();
    let r = hyper_client.request(request).await.unwrap();

    Ok::<_, String>(r)
}
