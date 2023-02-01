mod shared;

/// Dev cli, has the same cli interface but also includes a GUI tool,
/// 
#[tokio::main]
async fn main() {
    shared::start().await;
}
