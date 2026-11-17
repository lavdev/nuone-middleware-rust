#[tokio::main]
async fn main() {
    if let Err(error) = nuone_middleware::infrastructure::bootstrap::run().await {
        eprintln!("Nuone Middleware failed to start: {error}");
        std::process::exit(1);
    }
}
