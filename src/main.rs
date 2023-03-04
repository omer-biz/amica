use hyper::{server::conn::Http, service::service_fn, Body, Request};
use tokio::net::{TcpListener, TcpStream};

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

async fn handle_client(client_tcp_stream: TcpStream) {
    {
        let mut buf = [0; 64];
        let n = client_tcp_stream.peek(&mut buf).await.unwrap();

        let head = String::from_utf8_lossy(&buf[..n]);
        println!("Host: {}", head.split_whitespace().nth(1).unwrap());
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
