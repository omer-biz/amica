mod lua_engine;
mod proxy;

use hyper::{server::conn::Http, service::service_fn, Body, Request, Response};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use lua_engine::LuaEngine;
use proxy::{ProxyRequest, ProxyResponse};

// struct ProxyHandler;

pub async fn handle_client(mut client: TcpStream) {
    let mut buf = [0; 512];
    client.peek(&mut buf).await.unwrap();

    if buf.starts_with(&[67, 79, 78, 78, 69, 67, 84]) {
        let nbytes = client.read(&mut buf).await.unwrap();

        let head = String::from_utf8_lossy(&buf[..nbytes]);
        let host = head.split_whitespace().nth(1).unwrap();


        client.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();

    } else {
        let _ = Http::new()
            .serve_connection(client, service_fn(handle_http_request))
            .await;
    }
}


async fn handle_http_request(request: Request<Body>) -> Result<Response<Body>, String> {
    let lua_req_filter = r#"
    function on_http_request(req)
        print("request: ", req:uri())


        for k, v in pairs(req:headers()) do
           print(k, v)
        end

        return req
    end

    function on_http_response(res)
        print("body:", res:body())

        for k, v in pairs(res:headers()) do
           print(k .. ": " .. v)
        end

        res:set_body("<h1>This is from the prox</h1>\n"..res:body())
        headers = res:headers()
        headers["content-length"] = 63
        res:set_headers(headers)

        print("")
        for k, v in pairs(res:headers()) do
           print(k .. ": " .. v)
        end

        return res
    end
    "#;

    let lua_engine = LuaEngine::new();
    lua_engine.load(lua_req_filter).unwrap();

    let proxy_request = ProxyRequest::from(request).await;
    let req = lua_engine.call_on_http_request(proxy_request).unwrap();

    let hyper_client = hyper::Client::new();
    let response = hyper_client.request(req).await.unwrap();

    let proxy_response = ProxyResponse::from(response).await;
    let response = lua_engine.call_on_http_response(proxy_response).unwrap();

    Ok::<_, String>(response)
}
