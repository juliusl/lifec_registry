mod shared;

/// Main ACR Binary, does not include gui feature,
///
#[tokio::main]
async fn main() {
    crate::shared::start().await;
}
