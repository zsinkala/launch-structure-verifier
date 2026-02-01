use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use tower_http::cors::{CorsLayer, Any};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::types::{AnalyzeRequest, AnalyzeResponse};
use crate::api::cached_analyze::analyze_with_cache;
use crate::providers::helius::HeliusProvider;
use crate::providers::alchemy::AlchemyProvider;
use crate::cache::SimpleCache;

pub struct AppState {
    pub cache: Mutex<SimpleCache>,
    pub helius_api_key: String,
    pub alchemy_api_key: String,
}

pub async fn analyze_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, StatusCode> {
    println!("Received request for: {} on {}", request.address, request.chain);

    let mut cache = state.cache.lock().await;

    // Create provider based on chain
    let response = match request.chain.as_str() {
        "solana" => {
            let provider = HeliusProvider::new(state.helius_api_key.clone());
            analyze_with_cache(request, &provider, &mut cache).await
        }
        "base" | "ethereum" | "evm" => {
            let provider = AlchemyProvider::new(state.alchemy_api_key.clone(), &request.chain);
            analyze_with_cache(request, &provider, &mut cache).await
        }
        _ => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    Ok(Json(response))
}

pub async fn run_server(port: u16, helius_api_key: String, alchemy_api_key: String) {
    let state = Arc::new(AppState {
        cache: Mutex::new(SimpleCache::new()),
        helius_api_key,
        alchemy_api_key,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/v1/analyze", post(analyze_handler))
        .layer(cors)
        .with_state(state);

    // CRITICAL FIX: Bind to 0.0.0.0 instead of 127.0.0.1 for external access
    let addr = format!("0.0.0.0:{}", port);
    println!("🚀 Server running on http://{}", addr);
    println!("📊 Ready to analyze tokens on Solana and Base!");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap();

    axum::serve(listener, app)
        .await
        .unwrap();
}
