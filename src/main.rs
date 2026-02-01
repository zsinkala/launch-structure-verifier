use launch_structure_verifier::server::run_server;
use std::env;

#[tokio::main]
async fn main() {
    let helius_api_key = env::var("HELIUS_API_KEY")
        .expect("HELIUS_API_KEY environment variable must be set");
    
    let alchemy_api_key = env::var("ALCHEMY_API_KEY")
        .expect("ALCHEMY_API_KEY environment variable must be set");

    let port = 3000;
    
    run_server(port, helius_api_key, alchemy_api_key).await;
}
