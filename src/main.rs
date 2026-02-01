use launch_structure_verifier::server::run_server;
use std::env;

#[tokio::main]
async fn main() {
    let helius_api_key = env::var("HELIUS_API_KEY")
        .expect("HELIUS_API_KEY environment variable must be set");
    
    let alchemy_api_key = env::var("ALCHEMY_API_KEY")
        .expect("ALCHEMY_API_KEY environment variable must be set");

    // Read PORT from environment (Render provides this)
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid number");
    
    run_server(port, helius_api_key, alchemy_api_key).await;
}
