use hyper::{server::conn::Http, service::service_fn, Body, Request};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() {
    let address = "127.0.0.1:9001";
    let tcp_listener = TcpListener::bind(address).await.unwrap();
    println!("listening on {}", address);

    loop {
        let (client_tcp_stream, _sock_addr) = tcp_listener.accept().await.unwrap();
        tokio::spawn(async move {
            handle_client(client_tcp_stream).await;
        });
    }
}

async fn handle_connect_client(mut client: TcpStream, mut server: TcpStream) {
    let (mut server_rx, mut server_tx) = server.split();
    let (mut client_rx, mut client_tx) = client.split();

    let mut server_buf = [0; 4096];
    let mut client_buf = [0; 4096];
    loop {
        tokio::select! {
            Ok(n) = server_rx.read(&mut server_buf) => {
                if n == 0 {
                    break;
                }
                client_tx.write_all(&server_buf[..n]).await.unwrap();
            },
            Ok(n) = client_rx.read(&mut client_buf) => {
                if n == 0 {
                    break;
                }
                server_tx.write_all(&client_buf[..n]).await.unwrap();
            }
        }
    }
}

async fn handle_client(mut client_tcp_stream: TcpStream) {
    let mut buf = [0; 512];
    client_tcp_stream.peek(&mut buf).await.unwrap();

    // if buf starts with "CONNECT"
    if buf.starts_with(&[67, 79, 78, 78, 69, 67, 84]) {
        let n = client_tcp_stream.read(&mut buf).await.unwrap();

        let head = String::from_utf8_lossy(&buf[..n]);
        let mut chunks = head.split_whitespace();
        let method = chunks.next().unwrap();
        let host = chunks.next().unwrap();
        println!("method: {}, Host: {}", method, host);

        let server_tcp_stream = TcpStream::connect(host).await.unwrap();

        client_tcp_stream
            .write_all(b"HTTP/1.1 200 OK\r\n\r\n")
            .await
            .unwrap();

        return handle_connect_client(client_tcp_stream, server_tcp_stream).await;
    }

    let _ = Http::new()
        .serve_connection(
            client_tcp_stream,
            service_fn(|req: Request<Body>| async {
                let hyper_client = hyper::Client::new();
                let r = hyper_client.request(req).await.unwrap();

                Ok::<_, String>(r)
            }),
        )
        .await;
}
