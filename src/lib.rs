mod intermediate_proxy_data;
mod lua_engine;

use anyhow::Context;
use clap::Parser;
use hyper::{server::conn::Http, service::service_fn, Body, Request, Response};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use intermediate_proxy_data::{ProxyRequest, ProxyResponse};
use lua_engine::LuaEngine;

static mut LUA_CODE: Option<String> = None;

#[derive(Parser)]
pub struct Args {
    /// Optinal lua script to run on the the intermediate
    /// request and response.
    #[arg(short, long, value_name = "lua script")]
    filter_script: Option<PathBuf>,

    /// Address to bind to.
    #[arg(short, long, value_name = "ip:port")]
    address: String,
}

pub struct Proxy {
    args: Args,
}

impl Proxy {
    pub async fn start(args: Args) -> anyhow::Result<()> {
        Proxy { args }.run().await
    }

    async fn run(&self) -> anyhow::Result<()> {
        let tcp_listener = TcpListener::bind(&self.args.address)
            .await
            .with_context(|| format!("Can not bind to {}", self.args.address))?;
        println!("Listening on {}", self.args.address);

        loop {
            let (client, sock_addr) = tcp_listener.accept().await?;
            println!("client connected on: {}", sock_addr);

            if let Some(path) = &self.args.filter_script {
                unsafe { LUA_CODE = Some(std::fs::read_to_string(path)?) }
            }
            tokio::spawn(async move {
                if let Err(error) = handle_client(client).await {
                    eprintln!("Error: {}", error);
                }
            });
        }
    }
}

async fn handle_client(mut client: TcpStream) -> anyhow::Result<()> {
    let mut buf = [0; 1024];
    let nbytes = client.peek(&mut buf).await?;

    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    let _req_size = req.parse(&buf[..nbytes])?;

    let method = req
        .method
        .with_context(|| "Can't find the `Method` of the request")?
        .to_string();

    let host = headers
        .iter()
        .find(|header| header.name == "Host")
        .with_context(|| "Can't find `Host` in the request header")?;

    println!("host: {}", String::from_utf8_lossy(host.value));

    if method == "CONNECT" {
        let mut server =
            TcpStream::connect(String::from_utf8_lossy(host.value).to_string()).await?;
        client.read(&mut buf).await?;
        client.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
        tokio::io::copy_bidirectional(&mut server, &mut client).await?;
    } else {
        Http::new()
            .serve_connection(client, service_fn(handle_http_request))
            .await?;
    }

    Ok(())
}

async fn handle_http_request(request: Request<Body>) -> anyhow::Result<Response<Body>> {
    let no_lua_vm = unsafe { LUA_CODE.is_none() };

    let response = if no_lua_vm {
        make_http_request(request).await?
    } else {
        let lua_engine = LuaEngine::new();
        unsafe { lua_engine.load(LUA_CODE.as_ref().expect(""))? }

        let proxy_request = ProxyRequest::from(request).await?;
        let request = lua_engine.call_on_http_request(proxy_request)?;

        let response = make_http_request(request).await?;

        let proxy_response = ProxyResponse::from(response).await?;
        lua_engine.call_on_http_response(proxy_response)?
    };

    Ok::<_, anyhow::Error>(response)
}

async fn make_http_request(request: Request<Body>) -> hyper::Result<Response<Body>> {
    hyper::Client::new().request(request).await
}
