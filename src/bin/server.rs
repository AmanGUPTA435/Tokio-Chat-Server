#[tokio::main]
async fn main() {
    chat_stream_tokio::server::run_server().await;
}