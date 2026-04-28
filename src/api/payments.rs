use serde::{Deserialize, Serialize};
use serde_json::json;

const BASE_USDC_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
const ERC20_TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

#[derive(Clone, Debug, Deserialize)]
pub struct VerifyPaymentRequest {
    pub tx_hash: String,
    pub token_address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifyPaymentResponse {
    pub valid: bool,
    pub report_access_id: Option<String>,
    pub message: String,
    pub amount_usdc: Option<String>,
}

#[derive(Debug)]
pub enum PaymentVerificationError {
    InvalidInput,
    NetworkError(String),
    InvalidResponse(String),
    TransactionNotFound,
}

pub async fn verify_base_usdc_payment(
    alchemy_api_key: &str,
    receiving_wallet: &str,
    minimum_amount_microusd: u64,
    request: &VerifyPaymentRequest,
) -> Result<VerifyPaymentResponse, PaymentVerificationError> {
    if !is_valid_evm_address(&request.tx_hash, 66) || request.token_address.trim().is_empty() {
        return Err(PaymentVerificationError::InvalidInput);
    }

    let receipt = fetch_transaction_receipt(alchemy_api_key, &request.tx_hash).await?;
    let status = receipt.status.as_deref().unwrap_or_default();
    if status != "0x1" {
        return Ok(VerifyPaymentResponse {
            valid: false,
            report_access_id: None,
            message: "Transaction was not successful.".to_string(),
            amount_usdc: None,
        });
    }

    let receiving_wallet = receiving_wallet.to_ascii_lowercase();
    let Some(amount_microusd) = find_matching_usdc_transfer(&receipt, &receiving_wallet) else {
        return Ok(VerifyPaymentResponse {
            valid: false,
            report_access_id: None,
            message: "No matching Base USDC payment to the receiving wallet was found.".to_string(),
            amount_usdc: None,
        });
    };

    if amount_microusd < minimum_amount_microusd {
        return Ok(VerifyPaymentResponse {
            valid: false,
            report_access_id: None,
            message: format!(
                "Payment was below the required amount of {} USDC.",
                format_usdc(minimum_amount_microusd)
            ),
            amount_usdc: Some(format_usdc(amount_microusd)),
        });
    }

    Ok(VerifyPaymentResponse {
        valid: true,
        report_access_id: Some(format!(
            "report_{}_{}",
            normalize_tx_hash(&request.tx_hash),
            normalize_report_address(&request.token_address)
        )),
        message: "Payment verified.".to_string(),
        amount_usdc: Some(format_usdc(amount_microusd)),
    })
}

async fn fetch_transaction_receipt(
    alchemy_api_key: &str,
    tx_hash: &str,
) -> Result<TransactionReceipt, PaymentVerificationError> {
    let rpc_url = format!("https://base-mainnet.g.alchemy.com/v2/{}", alchemy_api_key);
    let request_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
    });

    let response = reqwest::Client::new()
        .post(rpc_url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| PaymentVerificationError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(PaymentVerificationError::InvalidResponse(format!(
            "Alchemy receipt request returned status {}",
            response.status()
        )));
    }

    let rpc_response: RpcResponse<TransactionReceipt> = response
        .json()
        .await
        .map_err(|e| PaymentVerificationError::InvalidResponse(e.to_string()))?;

    rpc_response.result.ok_or(PaymentVerificationError::TransactionNotFound)
}

fn find_matching_usdc_transfer(receipt: &TransactionReceipt, receiving_wallet: &str) -> Option<u64> {
    receipt.logs.iter().find_map(|log| {
        if normalize_address(&log.address) != normalize_address(BASE_USDC_ADDRESS) {
            return None;
        }

        let transfer_topic = log.topics.get(0)?;
        let to_topic = log.topics.get(2)?;
        if transfer_topic.to_ascii_lowercase() != ERC20_TRANSFER_TOPIC {
            return None;
        }

        let to_address = address_from_topic(to_topic)?;
        if normalize_address(&to_address) != normalize_address(receiving_wallet) {
            return None;
        }

        parse_hex_u64(&log.data)
    })
}

pub fn parse_usdc_amount_to_microusd(value: &str) -> Result<u64, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        return Err("USDC amount must be positive".to_string());
    }

    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fractional = parts.next().unwrap_or_default();
    if parts.next().is_some() || whole.is_empty() || fractional.len() > 6 {
        return Err("USDC amount can have at most 6 decimal places".to_string());
    }

    let whole_units = whole
        .parse::<u64>()
        .map_err(|_| "USDC amount must be numeric".to_string())?;
    let mut fractional_units = fractional.to_string();
    while fractional_units.len() < 6 {
        fractional_units.push('0');
    }
    let fractional_units = if fractional_units.is_empty() {
        0
    } else {
        fractional_units
            .parse::<u64>()
            .map_err(|_| "USDC amount must be numeric".to_string())?
    };

    whole_units
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(fractional_units))
        .ok_or_else(|| "USDC amount is too large".to_string())
}

fn format_usdc(amount_microusd: u64) -> String {
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

fn address_from_topic(topic: &str) -> Option<String> {
    let topic = topic.strip_prefix("0x")?;
    if topic.len() != 64 {
        return None;
    }
    Some(format!("0x{}", &topic[24..]))
}

fn normalize_address(address: &str) -> String {
    address.to_ascii_lowercase()
}

fn normalize_tx_hash(tx_hash: &str) -> String {
    tx_hash.trim_start_matches("0x").to_ascii_lowercase()
}

fn normalize_report_address(address: &str) -> String {
    address
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(48)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_valid_evm_address(value: &str, len: usize) -> bool {
    value.len() == len
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_hex_u64(value: &str) -> Option<u64> {
    u64::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct TransactionReceipt {
    status: Option<String>,
    logs: Vec<TransactionLog>,
}

#[derive(Debug, Deserialize)]
struct TransactionLog {
    address: String,
    topics: Vec<String>,
    data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usdc_amount_to_microusd() {
        assert_eq!(parse_usdc_amount_to_microusd("5").unwrap(), 5_000_000);
        assert_eq!(parse_usdc_amount_to_microusd("5.25").unwrap(), 5_250_000);
        assert_eq!(parse_usdc_amount_to_microusd("0.000001").unwrap(), 1);
        assert!(parse_usdc_amount_to_microusd("5.0000001").is_err());
        assert!(parse_usdc_amount_to_microusd("-1").is_err());
    }

    #[test]
    fn test_find_matching_usdc_transfer() {
        let receipt = TransactionReceipt {
            status: Some("0x1".to_string()),
            logs: vec![TransactionLog {
                address: BASE_USDC_ADDRESS.to_string(),
                topics: vec![
                    ERC20_TRANSFER_TOPIC.to_string(),
                    "0x0000000000000000000000001111111111111111111111111111111111111111".to_string(),
                    "0x0000000000000000000000002222222222222222222222222222222222222222".to_string(),
                ],
                data: "0x00000000000000000000000000000000000000000000000000000000004c4b40".to_string(),
            }],
        };

        assert_eq!(
            find_matching_usdc_transfer(
                &receipt,
                "0x2222222222222222222222222222222222222222"
            ),
            Some(5_000_000)
        );
    }
}
