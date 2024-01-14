mod intermediate_proxy_data;
pub mod lua_engine;

use anyhow::Context;
use clap::Parser;
use hyper::{server::conn::Http, service::service_fn, Body, Request, Response};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use intermediate_proxy_data::{ProxyRequest, ProxyResponse};
use lua_engine::{LuaPool, Messenger};

#[derive(Parser)]
pub struct Args {
    /// Optinal lua script to run on the the intermediate
    /// request and response.
    #[arg(short, long, value_name = "lua script")]
    filter_script: Option<PathBuf>,

    /// Address to bind to.
    #[arg(short, long, value_name = "ip:port", default_value = "localhost:9001")]
    address: String,

    /// Number of Proxy pools to spwan.
    #[arg(short, long, value_name = "pool number", default_value = "1")]
    pool_number: usize,

    /// Verbosity. if turned on shows the request and response as they are
    /// happening for `http` requests.
    #[arg(short, long)]
    verbose: bool,
}

pub struct Proxy {
    args: Args,
}

impl Proxy {
    pub async fn start(args: Args) -> anyhow::Result<()> {
        Proxy { args }.run().await
    }

    async fn run(self) -> anyhow::Result<()> {
        let address = self.args.address;
        let mut lua_msgr = None;

        if let Some(path) = self.args.filter_script {
            let pool_number = self.args.pool_number;
            let (_, msgr) = LuaPool::build(pool_number, path)?;
            lua_msgr = Some(msgr);
        }

        let tcp_listener = TcpListener::bind(&address)
            .await
            .with_context(|| format!("Can not bind to {}", address))?;
        if self.args.verbose {
            println!("Listening on {}", address);
        }

        loop {
            let (client, sock_addr) = tcp_listener.accept().await?;
            if self.args.verbose {
                println!("Request coming from: {}", sock_addr);
            }

            let lua_msgr = lua_msgr.clone();

            tokio::spawn(async move {
                if let Err(error) = handle_client(client, lua_msgr, self.args.verbose).await {
                    eprintln!("Handle Client Error: {:#?}", error);
                }
            });
        }
    }
}

async fn handle_client(
    mut client: TcpStream,
    mut lua_msgr: Option<Messenger>,
    verbose: bool,
) -> anyhow::Result<()> {
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

    if method == "CONNECT" {
        let mut server =
            TcpStream::connect(String::from_utf8_lossy(host.value).to_string()).await?;
        client.read_exact(&mut buf[..nbytes]).await?;
        client.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
        tokio::io::copy_bidirectional(&mut server, &mut client).await?;
    } else {
        Http::new()
            .serve_connection(
                client,
                service_fn(|req| handle_http_request(req, lua_msgr.take(), verbose)),
            )
            .await?;
    }

    Ok(())
}

async fn handle_http_request(
    request: Request<Body>,
    lua_msgr: Option<Messenger>,
    verbose: bool,
) -> anyhow::Result<Response<Body>> {
    if verbose {
        print_request(&request);
    }
    let response = if let Some(lua_msgr) = lua_msgr {
        let proxy_request = ProxyRequest::from(request).await?;
        let request = lua_msgr.call_on_http_request(proxy_request).await?;

        let response = make_http_request(request).await?;

        let proxy_response = ProxyResponse::from(response).await?;
        lua_msgr.call_on_http_response(proxy_response).await?
    } else {
        make_http_request(request).await?
    };
    if verbose {
        print_response(&response);
    }

    Ok::<_, anyhow::Error>(response)
}

async fn make_http_request(request: Request<Body>) -> hyper::Result<Response<Body>> {
    hyper::Client::new().request(request).await
}

fn print_response(response: &Response<Body>) {
    println!("<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<");
    println!("< status: {}", response.status());
    println!("< version: {:?}", response.version());

    println!("> headers: ");
    for (k, v) in response.headers() {
        println!("> \t{}: {:?}", k, v);
    }

    println!("<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<\n");
}

fn print_request(request: &Request<Body>) {
    println!(">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>");
    println!("> method: {}", request.method());
    println!("> uri: {}", request.uri());
    println!("> version: {:?}", request.version());

    println!("> headers: ");
    for (k, v) in request.headers() {
        println!("> \t{}: {:?}", k, v);
    }
    println!(">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>\n");
}
