use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::{get, post},
    Json, Router,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::api::cached_analyze::analyze_with_cache;
use crate::api::payments::{
    verify_base_usdc_payment, PaymentVerificationError, VerifyPaymentRequest, VerifyPaymentResponse,
};
use crate::api::types::{AnalyzeRequest, AnalyzeResponse};
use crate::cache::SimpleCache;
use crate::payments_store::{SupabasePaymentStore, UsedPaymentTxRecord};
use crate::providers::alchemy::AlchemyProvider;
use crate::providers::helius::HeliusProvider;

const PAYMENT_LOG_LIMIT: usize = 100;

pub struct AppState {
    pub cache: Mutex<SimpleCache>,
    pub rate_limiter: Mutex<RateLimiter>,
    pub used_payment_txs: Mutex<HashSet<String>>,
    pub payment_verification_logs: Mutex<VecDeque<PaymentVerificationLogEntry>>,
    pub helius_api_key: String,
    pub alchemy_api_key: String,
    pub payment_wallet_address: Option<String>,
    pub paid_report_price_microusd: u64,
    pub payment_store: Option<SupabasePaymentStore>,
    pub admin_api_key: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PaymentVerificationLogEntry {
    pub observed_at_unix: u64,
    pub tx_hash: String,
    pub token_address: String,
    pub status: String,
    pub http_status: u16,
    pub valid: bool,
    pub message: String,
    pub amount_usdc: Option<String>,
    pub report_access_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AdminPaymentStatusResponse {
    pub payment_wallet_configured: bool,
    pub persistent_store_configured: bool,
    pub paid_report_price_usdc: String,
    pub retained_attempts: usize,
    pub attempts: Vec<PaymentVerificationLogEntry>,
}

pub async fn analyze_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, StatusCode> {
    println!(
        "Received request for: {} on {}",
        request.address, request.chain
    );

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

pub async fn verify_payment_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<VerifyPaymentRequest>,
) -> Result<Json<VerifyPaymentResponse>, StatusCode> {
    let client_id = client_id_from_headers(&headers, remote_addr);
    let mut rate_limiter = state.rate_limiter.lock().await;
    if !rate_limiter.allow_request(&client_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    drop(rate_limiter);

    if !is_valid_evm_address(&request.tx_hash, 66) || request.token_address.trim().is_empty() {
        log_payment_attempt(
            &state,
            payment_log_from_request(
                &request,
                "invalid_input",
                StatusCode::BAD_REQUEST,
                false,
                "Invalid transaction hash or token address.",
                None,
                None,
            ),
        )
        .await;
        return Err(StatusCode::BAD_REQUEST);
    }

    let Some(payment_wallet_address) = &state.payment_wallet_address else {
        log_payment_attempt(
            &state,
            payment_log_from_request(
                &request,
                "not_configured",
                StatusCode::SERVICE_UNAVAILABLE,
                false,
                "Payment verification is not configured.",
                None,
                None,
            ),
        )
        .await;
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let normalized_tx_hash = request.tx_hash.to_ascii_lowercase();
    let tx_was_used = match payment_tx_was_used(&state, &normalized_tx_hash).await {
        Ok(tx_was_used) => tx_was_used,
        Err(status) => {
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "store_lookup_error",
                    status,
                    false,
                    "Payment store lookup failed.",
                    None,
                    None,
                ),
            )
            .await;
            return Err(status);
        }
    };

    if tx_was_used {
        let response = VerifyPaymentResponse {
            valid: false,
            report_access_id: None,
            message: "This transaction hash has already been used.".to_string(),
            amount_usdc: None,
        };
        log_payment_attempt(
            &state,
            payment_log_from_response(&request, "already_used", StatusCode::OK, &response),
        )
        .await;
        return Ok(Json(response));
    }

    let response = match verify_base_usdc_payment(
        &state.alchemy_api_key,
        payment_wallet_address,
        state.paid_report_price_microusd,
        &request,
    )
    .await
    {
        Ok(response) => response,
        Err(PaymentVerificationError::InvalidInput) => {
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "invalid_input",
                    StatusCode::BAD_REQUEST,
                    false,
                    "Invalid payment verification input.",
                    None,
                    None,
                ),
            )
            .await;
            return Err(StatusCode::BAD_REQUEST);
        }
        Err(PaymentVerificationError::TransactionNotFound) => {
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "transaction_not_found",
                    StatusCode::NOT_FOUND,
                    false,
                    "Transaction was not found on Base.",
                    None,
                    None,
                ),
            )
            .await;
            return Err(StatusCode::NOT_FOUND);
        }
        Err(PaymentVerificationError::NetworkError(message)) => {
            eprintln!("Payment verification network error: {}", message);
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "network_error",
                    StatusCode::BAD_GATEWAY,
                    false,
                    "Payment verification network error.",
                    None,
                    None,
                ),
            )
            .await;
            return Err(StatusCode::BAD_GATEWAY);
        }
        Err(PaymentVerificationError::InvalidResponse(message)) => {
            eprintln!("Payment verification invalid RPC response: {}", message);
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "invalid_provider_response",
                    StatusCode::BAD_GATEWAY,
                    false,
                    "Payment provider returned an invalid response.",
                    None,
                    None,
                ),
            )
            .await;
            return Err(StatusCode::BAD_GATEWAY);
        }
    };

    if response.valid {
        if let Err(status) = store_used_payment_tx(
            &state,
            UsedPaymentTxRecord {
                tx_hash: normalized_tx_hash,
                report_access_id: response.report_access_id.clone().unwrap_or_default(),
                token_address: request.token_address.clone(),
                amount_usdc: response.amount_usdc.clone(),
            },
        )
        .await
        {
            log_payment_attempt(
                &state,
                payment_log_from_request(
                    &request,
                    "store_insert_error",
                    status,
                    false,
                    "Payment verified but could not be stored as used.",
                    response.amount_usdc.clone(),
                    response.report_access_id.clone(),
                ),
            )
            .await;
            return Err(status);
        }
    }

    let status = if response.valid {
        "verified"
    } else {
        "rejected"
    };
    log_payment_attempt(
        &state,
        payment_log_from_response(&request, status, StatusCode::OK, &response),
    )
    .await;

    Ok(Json(response))
}

pub async fn admin_payment_status_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AdminPaymentStatusResponse>, StatusCode> {
    require_admin_api_key(&state, &headers)?;

    let logs = state.payment_verification_logs.lock().await;
    let attempts = logs.iter().rev().cloned().collect::<Vec<_>>();

    Ok(Json(AdminPaymentStatusResponse {
        payment_wallet_configured: state.payment_wallet_address.is_some(),
        persistent_store_configured: state.payment_store.is_some(),
        paid_report_price_usdc: format_usdc_amount(state.paid_report_price_microusd),
        retained_attempts: logs.len(),
        attempts,
    }))
}

async fn health_handler() -> &'static str {
    "ok"
}

pub async fn run_server(
    port: u16,
    helius_api_key: String,
    alchemy_api_key: String,
    frontend_origin: Option<String>,
    payment_wallet_address: Option<String>,
    paid_report_price_microusd: u64,
    payment_store: Option<SupabasePaymentStore>,
    admin_api_key: Option<String>,
) {
    let state = Arc::new(AppState {
        cache: Mutex::new(SimpleCache::new()),
        rate_limiter: Mutex::new(RateLimiter::new(60, 60)),
        used_payment_txs: Mutex::new(HashSet::new()),
        payment_verification_logs: Mutex::new(VecDeque::new()),
        helius_api_key,
        alchemy_api_key,
        payment_wallet_address,
        paid_report_price_microusd,
        payment_store,
        admin_api_key,
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
        .route("/api/v1/payments/verify", post(verify_payment_handler))
        .route("/api/v1/admin/payments", get(admin_payment_status_handler))
        .layer(cors)
        .with_state(state);

    // CRITICAL FIX: Bind to 0.0.0.0 instead of 127.0.0.1 for external access
    let addr = format!("0.0.0.0:{}", port);
    println!("🚀 Server running on http://{}", addr);
    println!("📊 Ready to analyze tokens on Solana and Base!");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn payment_tx_was_used(
    state: &AppState,
    normalized_tx_hash: &str,
) -> Result<bool, StatusCode> {
    if let Some(payment_store) = &state.payment_store {
        return payment_store
            .has_used_tx(normalized_tx_hash)
            .await
            .map_err(|error| {
                eprintln!("Payment store lookup error: {:?}", error);
                StatusCode::BAD_GATEWAY
            });
    }

    let used_payment_txs = state.used_payment_txs.lock().await;
    Ok(used_payment_txs.contains(normalized_tx_hash))
}

async fn store_used_payment_tx(
    state: &AppState,
    record: UsedPaymentTxRecord,
) -> Result<(), StatusCode> {
    if let Some(payment_store) = &state.payment_store {
        return payment_store.store_used_tx(record).await.map_err(|error| {
            eprintln!("Payment store insert error: {:?}", error);
            StatusCode::BAD_GATEWAY
        });
    }

    let mut used_payment_txs = state.used_payment_txs.lock().await;
    used_payment_txs.insert(record.tx_hash);
    Ok(())
}

async fn log_payment_attempt(state: &AppState, entry: PaymentVerificationLogEntry) {
    let mut logs = state.payment_verification_logs.lock().await;
    if logs.len() >= PAYMENT_LOG_LIMIT {
        logs.pop_front();
    }
    logs.push_back(entry);
}

fn payment_log_from_response(
    request: &VerifyPaymentRequest,
    status: &str,
    http_status: StatusCode,
    response: &VerifyPaymentResponse,
) -> PaymentVerificationLogEntry {
    payment_log_from_request(
        request,
        status,
        http_status,
        response.valid,
        &response.message,
        response.amount_usdc.clone(),
        response.report_access_id.clone(),
    )
}

fn payment_log_from_request(
    request: &VerifyPaymentRequest,
    status: &str,
    http_status: StatusCode,
    valid: bool,
    message: &str,
    amount_usdc: Option<String>,
    report_access_id: Option<String>,
) -> PaymentVerificationLogEntry {
    PaymentVerificationLogEntry {
        observed_at_unix: current_timestamp(),
        tx_hash: request.tx_hash.to_ascii_lowercase(),
        token_address: request.token_address.clone(),
        status: status.to_string(),
        http_status: http_status.as_u16(),
        valid,
        message: message.to_string(),
        amount_usdc,
        report_access_id,
    }
}

fn require_admin_api_key(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected_key) = state.admin_api_key.as_deref().filter(|key| !key.is_empty()) else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let provided_key = headers
        .get("x-admin-api-key")
        .and_then(|value| value.to_str().ok())
        .or_else(|| bearer_token_from_headers(headers));

    match provided_key {
        Some(provided_key)
            if constant_time_eq(provided_key.as_bytes(), expected_key.as_bytes()) =>
        {
            Ok(())
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.iter()
        .zip(right.iter())
        .fold(0, |diff, (left, right)| diff | (left ^ right))
        == 0
}

fn format_usdc_amount(amount_microusd: u64) -> String {
    let whole = amount_microusd / 1_000_000;
    let fractional = amount_microusd % 1_000_000;
    if fractional == 0 {
        return whole.to_string();
    }

    let mut fractional = format!("{:06}", fractional);
    while fractional.ends_with('0') {
        fractional.pop();
    }
    format!("{}.{}", whole, fractional)
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
        "base" | "ethereum" | "evm" => is_valid_evm_address(address, 42),
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

fn is_valid_evm_address(address: &str, len: usize) -> bool {
    address.len() == len
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
        assert!(!is_valid_request_address(
            "solana",
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
        ));
        assert!(!is_valid_request_address("base", "not-an-address"));
        assert!(!is_valid_request_address(
            "unknown",
            "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"
        ));
    }

    #[test]
    fn test_rate_limiter_blocks_after_limit() {
        let mut limiter = RateLimiter::new(2, 60);

        assert!(limiter.allow_request("127.0.0.1"));
        assert!(limiter.allow_request("127.0.0.1"));
        assert!(!limiter.allow_request("127.0.0.1"));
        assert!(limiter.allow_request("127.0.0.2"));
    }

    #[test]
    fn test_admin_api_key_validation_accepts_header_and_bearer() {
        let state = AppState {
            cache: Mutex::new(SimpleCache::new()),
            rate_limiter: Mutex::new(RateLimiter::new(60, 60)),
            used_payment_txs: Mutex::new(HashSet::new()),
            payment_verification_logs: Mutex::new(VecDeque::new()),
            helius_api_key: "helius".to_string(),
            alchemy_api_key: "alchemy".to_string(),
            payment_wallet_address: None,
            paid_report_price_microusd: 5_000_000,
            payment_store: None,
            admin_api_key: Some("secret-admin-key".to_string()),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-admin-api-key",
            HeaderValue::from_static("secret-admin-key"),
        );
        assert!(require_admin_api_key(&state, &headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer secret-admin-key"),
        );
        assert!(require_admin_api_key(&state, &headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert("x-admin-api-key", HeaderValue::from_static("wrong-key"));
        assert_eq!(
            require_admin_api_key(&state, &headers),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn test_admin_api_key_validation_requires_configured_key() {
        let state = AppState {
            cache: Mutex::new(SimpleCache::new()),
            rate_limiter: Mutex::new(RateLimiter::new(60, 60)),
            used_payment_txs: Mutex::new(HashSet::new()),
            payment_verification_logs: Mutex::new(VecDeque::new()),
            helius_api_key: "helius".to_string(),
            alchemy_api_key: "alchemy".to_string(),
            payment_wallet_address: None,
            paid_report_price_microusd: 5_000_000,
            payment_store: None,
            admin_api_key: None,
        };

        assert_eq!(
            require_admin_api_key(&state, &HeaderMap::new()),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    #[tokio::test]
    async fn test_payment_verification_log_keeps_latest_entries() {
        let state = AppState {
            cache: Mutex::new(SimpleCache::new()),
            rate_limiter: Mutex::new(RateLimiter::new(60, 60)),
            used_payment_txs: Mutex::new(HashSet::new()),
            payment_verification_logs: Mutex::new(VecDeque::new()),
            helius_api_key: "helius".to_string(),
            alchemy_api_key: "alchemy".to_string(),
            payment_wallet_address: None,
            paid_report_price_microusd: 5_000_000,
            payment_store: None,
            admin_api_key: Some("secret-admin-key".to_string()),
        };

        for index in 0..(PAYMENT_LOG_LIMIT + 3) {
            log_payment_attempt(
                &state,
                PaymentVerificationLogEntry {
                    observed_at_unix: index as u64,
                    tx_hash: format!("0x{:064}", index),
                    token_address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
                    status: "rejected".to_string(),
                    http_status: 200,
                    valid: false,
                    message: "Payment rejected.".to_string(),
                    amount_usdc: None,
                    report_access_id: None,
                },
            )
            .await;
        }

        let logs = state.payment_verification_logs.lock().await;
        assert_eq!(logs.len(), PAYMENT_LOG_LIMIT);
        assert_eq!(logs.front().unwrap().observed_at_unix, 3);
        assert_eq!(
            logs.back().unwrap().observed_at_unix,
            (PAYMENT_LOG_LIMIT + 2) as u64
        );
    }
}
