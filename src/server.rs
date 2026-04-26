use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::{CorsLayer, Any};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::api::types::{AnalyzeRequest, AnalyzeResponse};
use crate::api::cached_analyze::analyze_with_cache;
use crate::providers::helius::HeliusProvider;
use crate::providers::alchemy::AlchemyProvider;
use crate::cache::SimpleCache;

pub struct AppState {
    pub cache: Mutex<SimpleCache>,
    pub rate_limiter: Mutex<RateLimiter>,
    pub helius_api_key: String,
    pub alchemy_api_key: String,
}

pub async fn analyze_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, StatusCode> {
    println!("Received request for: {} on {}", request.address, request.chain);

    let client_id = client_id_from_headers(&headers, remote_addr);
    let mut rate_limiter = state.rate_limiter.lock().await;
    if !rate_limiter.allow_request(&client_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    drop(rate_limiter);

    if !is_valid_request_address(&request.chain, &request.address) {
        return Err(StatusCode::BAD_REQUEST);
    }

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

async fn health_handler() -> &'static str {
    "ok"
}

pub async fn run_server(
    port: u16,
    helius_api_key: String,
    alchemy_api_key: String,
    frontend_origin: Option<String>,
) {
    let state = Arc::new(AppState {
        cache: Mutex::new(SimpleCache::new()),
        rate_limiter: Mutex::new(RateLimiter::new(60, 60)),
        helius_api_key,
        alchemy_api_key,
    });

    let cors = match frontend_origin {
        Some(origin) => {
            let origin = origin
                .parse::<HeaderValue>()
                .expect("FRONTEND_ORIGIN must be a valid HTTP header value");
            CorsLayer::new().allow_origin(origin)
        }
        None => {
            println!("No FRONTEND_ORIGIN set; allowing CORS from any origin.");
            CorsLayer::new().allow_origin(Any)
        }
    }
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
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

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

pub struct RateLimiter {
    requests: HashMap<String, VecDeque<u64>>,
    max_requests: usize,
    window_seconds: u64,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: HashMap::new(),
            max_requests,
            window_seconds,
        }
    }

    pub fn allow_request(&mut self, client_id: &str) -> bool {
        let now = current_timestamp();
        let window_start = now.saturating_sub(self.window_seconds);
        let client_requests = self.requests.entry(client_id.to_string()).or_default();

        while let Some(timestamp) = client_requests.front() {
            if *timestamp < window_start {
                client_requests.pop_front();
            } else {
                break;
            }
        }

        if client_requests.len() >= self.max_requests {
            return false;
        }

        client_requests.push_back(now);
        true
    }
}

fn client_id_from_headers(headers: &HeaderMap, remote_addr: SocketAddr) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| remote_addr.ip().to_string())
}

fn is_valid_request_address(chain: &str, address: &str) -> bool {
    let address = address.trim();

    match chain {
        "solana" => is_valid_solana_address(address),
        "base" | "ethereum" | "evm" => is_valid_evm_address(address),
        _ => false,
    }
}

fn is_valid_solana_address(address: &str) -> bool {
    let len = address.len();
    (32..=44).contains(&len)
        && address
            .bytes()
            .all(|byte| matches!(byte, b'1'..=b'9' | b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'Z' | b'a'..=b'k' | b'm'..=b'z'))
}

fn is_valid_evm_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("0x")
        && address[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_validation() {
        assert!(is_valid_request_address(
            "solana",
            "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"
        ));
        assert!(is_valid_request_address(
            "base",
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
        ));
        assert!(!is_valid_request_address("solana", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"));
        assert!(!is_valid_request_address("base", "not-an-address"));
        assert!(!is_valid_request_address("unknown", "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"));
    }

    #[test]
    fn test_rate_limiter_blocks_after_limit() {
        let mut limiter = RateLimiter::new(2, 60);

        assert!(limiter.allow_request("127.0.0.1"));
        assert!(limiter.allow_request("127.0.0.1"));
        assert!(!limiter.allow_request("127.0.0.1"));
        assert!(limiter.allow_request("127.0.0.2"));
    }
}
