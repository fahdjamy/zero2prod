use std::net::TcpListener;
use zero2prod::run;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let port = "8001";
    let listener = TcpListener::bind("127.0.0.1:8001")
        .expect(format!("failed to bind on {:?}", port).as_str());
    run(listener)?.await
}
