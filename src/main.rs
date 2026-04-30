use launch_structure_verifier::api::payments::parse_usdc_amount_to_microusd;
use launch_structure_verifier::payments_store::SupabasePaymentStore;
use launch_structure_verifier::server::run_server;
use std::env;

#[tokio::main]
async fn main() {
    let helius_api_key =
        env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY environment variable must be set");

    let alchemy_api_key =
        env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY environment variable must be set");

    // Read PORT from environment (Render provides this)
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid number");

    let frontend_origin = env::var("FRONTEND_ORIGIN").ok();
    let payment_wallet_address = env::var("PAYMENT_WALLET_ADDRESS").ok();
    let admin_api_key = env::var("ADMIN_API_KEY").ok();
    let paid_report_price_usdc =
        env::var("PAID_REPORT_PRICE_USDC").unwrap_or_else(|_| "5".to_string());
    let paid_report_price_microusd = parse_usdc_amount_to_microusd(&paid_report_price_usdc)
        .expect("PAID_REPORT_PRICE_USDC must be a positive USDC amount");
    let payment_store = match (
        env::var("SUPABASE_URL").ok(),
        env::var("SUPABASE_SERVICE_ROLE_KEY").ok(),
    ) {
        (Some(url), Some(service_role_key)) => {
            Some(SupabasePaymentStore::new(url, service_role_key))
        }
        _ => None,
    };

    run_server(
        port,
        helius_api_key,
        alchemy_api_key,
        frontend_origin,
        payment_wallet_address,
        paid_report_price_microusd,
        payment_store,
        admin_api_key,
    )
    .await;
}
