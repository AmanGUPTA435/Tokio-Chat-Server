#[tokio::main]
async fn main() {
    chat_stream_tokio::client::run_client().await;
}