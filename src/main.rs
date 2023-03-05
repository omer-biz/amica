use amica::handle_client;
use tokio::net::TcpListener;

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
