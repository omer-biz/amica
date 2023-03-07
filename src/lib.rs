mod lua_engine;
mod proxy;

use hyper::{server::conn::Http, service::service_fn, Body, Request, Response};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use lua_engine::LuaEngine;
use proxy::ProxyRequest;

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

async fn handle_http_request(request: Request<Body>) -> Result<Response<Body>, String> {
    let lua_req_filter = r#"
    function on_http_request(req)
        print("request: ", req:uri())
        req:set_uri("http://www.duckduckgo.com")
        print("request: ", req:uri())
        return req
    end
    "#;

    let lua_engine = LuaEngine::new();
    lua_engine.load(lua_req_filter).unwrap();

    let proxy_request = ProxyRequest::from(request).await;
    let request: Request<Body> = lua_engine
        .call_on_http_request(proxy_request)
        .unwrap()
        .into();

    let hyper_client = hyper::Client::new();
    let r = hyper_client.request(request).await.unwrap();

    Ok::<_, String>(r)
}
