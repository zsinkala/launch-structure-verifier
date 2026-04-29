use super::{ProviderError, TokenProvider};
use crate::types::*;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

pub struct AlchemyProvider {
    basescan_api_key: Option<String>,
    chain: String,
    rpc_url: String,
}

impl AlchemyProvider {
    pub fn new(api_key: String, chain: &str) -> Self {
        let rpc_url = match chain {
            "base" => format!("https://base-mainnet.g.alchemy.com/v2/{}", api_key),
            "ethereum" => format!("https://eth-mainnet.g.alchemy.com/v2/{}", api_key),
            _ => format!("https://base-mainnet.g.alchemy.com/v2/{}", api_key),
        };

        let basescan_api_key = std::env::var("BASESCAN_API_KEY").ok();

        Self {
            basescan_api_key,
            chain: chain.to_string(),
            rpc_url,
        }
    }

    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, ProviderError> {
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_url)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eprintln!("RPC Error - Status: {}, Body: {}", status, body);
            return Err(ProviderError::InvalidResponse);
        }

        let text = response
            .text()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        eprintln!("RPC Response: {}", text);

        let rpc_response: RpcResponse<T> = serde_json::from_str(&text).map_err(|e| {
            eprintln!("JSON Parse Error: {}", e);
            ProviderError::InvalidResponse
        })?;

        rpc_response.result.ok_or(ProviderError::InvalidResponse)
    }
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BlockResponse {
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct BasescanResponse<T> {
    status: String,
    message: String,
    result: T,
}

#[derive(Debug, Deserialize)]
struct BasescanTokenHolder {
    #[serde(alias = "TokenHolderAddress", alias = "tokenHolderAddress")]
    address: String,
    #[serde(alias = "TokenHolderQuantity", alias = "tokenHolderQuantity")]
    quantity: String,
}

#[async_trait]
impl TokenProvider for AlchemyProvider {
    fn provider_name(&self) -> &str {
        "alchemy"
    }

    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError> {
        // ERC20 decimals() function signature: 0x313ce567
        let decimals_data = "0x313ce567";

        // Call decimals()
        let decimals_result: String = self
            .rpc_call(
                "eth_call",
                json!([
                    {
                        "to": address,
                        "data": decimals_data
                    },
                    "latest"
                ]),
            )
            .await?;

        let decimals = if decimals_result.len() > 2 {
            u8::from_str_radix(&decimals_result[2..], 16).ok()
        } else {
            None
        };

        Ok(Metadata {
            name: None,
            symbol: None,
            decimals,
            standard: TokenStandard::Erc20,
        })
    }

    async fn fetch_supply(&self, address: &str) -> Result<SupplyInfo, ProviderError> {
        // ERC20 totalSupply() function signature: 0x18160ddd
        let total_supply_data = "0x18160ddd";

        let supply_hex: String = self
            .rpc_call(
                "eth_call",
                json!([
                    {
                        "to": address,
                        "data": total_supply_data
                    },
                    "latest"
                ]),
            )
            .await?;

        let total_supply_raw = supply_hex.trim_start_matches("0x").to_string();

        // Convert hex to decimal
        let total_supply = if let Ok(raw) = u128::from_str_radix(&total_supply_raw, 16) {
            // Assume 18 decimals for now (standard ERC20)
            Some(raw as f64 / 1e18)
        } else {
            None
        };

        Ok(SupplyInfo {
            total_supply_raw: Some(supply_hex),
            total_supply,
        })
    }

    async fn fetch_authorities(&self, address: &str) -> Result<AuthorityInfo, ProviderError> {
        // ERC20 owner() function signature: 0x8da5cb5b
        let owner_data = "0x8da5cb5b";

        let owner_result: String = self
            .rpc_call(
                "eth_call",
                json!([
                    {
                        "to": address,
                        "data": owner_data
                    },
                    "latest"
                ]),
            )
            .await
            .unwrap_or_else(|_| "0x".to_string());

        // Extract address from result (last 40 chars)
        let owner = if owner_result.len() >= 42 {
            let addr = format!("0x{}", &owner_result[owner_result.len() - 40..]);

            // Check if owner is zero address or burn address
            if addr == "0x0000000000000000000000000000000000000000"
                || addr == "0x000000000000000000000000000000000000dead"
            {
                None
            } else {
                Some(addr)
            }
        } else {
            None
        };

        let mint_mutable = owner.is_some();

        Ok(AuthorityInfo {
            mint_authority: None,   // EVM doesn't use this concept
            freeze_authority: None, // EVM doesn't use this concept
            owner,
            mint_mutable: Some(mint_mutable),
        })
    }

    async fn fetch_holders(
        &self,
        address: &str,
        limit: usize,
    ) -> Result<HolderInfo, ProviderError> {
        if self.chain != "base" {
            return Ok(unknown_holder_info());
        }

        let Some(api_key) = &self.basescan_api_key else {
            return Ok(unknown_holder_info());
        };

        let supply_raw = self.fetch_total_supply_raw(address).await?;
        let holders = fetch_basescan_holders(address, limit, api_key).await?;

        build_holder_info(holders, &supply_raw, limit)
    }

    async fn fetch_creation_time(&self, address: &str) -> Result<CreationInfo, ProviderError> {
        let created_at_unix = self.fetch_contract_creation_block_time(address).await?;
        build_creation_info(created_at_unix)
    }
}

impl AlchemyProvider {
    async fn fetch_total_supply_raw(&self, address: &str) -> Result<String, ProviderError> {
        let supply = self.fetch_supply(address).await?;
        let raw_hex = supply
            .total_supply_raw
            .ok_or(ProviderError::InvalidResponse)?;
        let raw = parse_hex_u128(&raw_hex)?;

        Ok(raw.to_string())
    }

    async fn fetch_contract_creation_block_time(
        &self,
        address: &str,
    ) -> Result<u64, ProviderError> {
        let latest_block_hex: String = self.rpc_call("eth_blockNumber", json!([])).await?;
        let latest_block = parse_hex_u64(&latest_block_hex)?;

        let latest_code = self.fetch_code_at_block(address, latest_block).await?;
        if !has_contract_code(&latest_code) {
            return Err(ProviderError::NotFound);
        }

        let mut low = 0;
        let mut high = latest_block;

        while low < high {
            let mid = low + ((high - low) / 2);
            let code = self.fetch_code_at_block(address, mid).await?;

            if has_contract_code(&code) {
                high = mid;
            } else {
                low = mid + 1;
            }
        }

        let block: BlockResponse = self
            .rpc_call(
                "eth_getBlockByNumber",
                json!([format_block_tag(low), false]),
            )
            .await?;

        parse_hex_u64(&block.timestamp)
    }

    async fn fetch_code_at_block(
        &self,
        address: &str,
        block: u64,
    ) -> Result<String, ProviderError> {
        self.rpc_call("eth_getCode", json!([address, format_block_tag(block)]))
            .await
    }
}

fn has_contract_code(code: &str) -> bool {
    let trimmed = code.trim();
    trimmed != "0x" && trimmed != "0x0" && trimmed.len() > 2
}

fn format_block_tag(block: u64) -> String {
    format!("0x{:x}", block)
}

fn parse_hex_u64(value: &str) -> Result<u64, ProviderError> {
    u64::from_str_radix(value.trim_start_matches("0x"), 16)
        .map_err(|_| ProviderError::InvalidResponse)
}

fn parse_hex_u128(value: &str) -> Result<u128, ProviderError> {
    u128::from_str_radix(value.trim_start_matches("0x"), 16)
        .map_err(|_| ProviderError::InvalidResponse)
}

async fn fetch_basescan_holders(
    address: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<BasescanTokenHolder>, ProviderError> {
    let offset = limit.max(5).to_string();
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.basescan.org/api")
        .query(&[
            ("module", "token"),
            ("action", "tokenholderlist"),
            ("contractaddress", address),
            ("page", "1"),
            ("offset", offset.as_str()),
            ("apikey", api_key),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ProviderError::InvalidResponse);
    }

    let body: BasescanResponse<Vec<BasescanTokenHolder>> = response
        .json()
        .await
        .map_err(|_| ProviderError::InvalidResponse)?;

    if body.status != "1" {
        eprintln!("BaseScan holder lookup failed: {}", body.message);
        return Err(ProviderError::InvalidResponse);
    }

    Ok(body.result)
}

fn build_holder_info(
    holders: Vec<BasescanTokenHolder>,
    supply_raw: &str,
    limit: usize,
) -> Result<HolderInfo, ProviderError> {
    let supply = parse_decimal_amount(supply_raw)?;
    if supply <= 0.0 {
        return Err(ProviderError::InvalidResponse);
    }

    let mut top_holders = Vec::new();
    for holder in holders.iter().take(limit) {
        let balance_raw = parse_decimal_amount(&holder.quantity)?;
        let pct_of_supply = Some((balance_raw / supply) * 100.0);

        top_holders.push(HolderBalance {
            address: holder.address.clone(),
            balance_raw: holder.quantity.clone(),
            balance: None,
            pct_of_supply,
        });
    }

    let top1_pct = top_holders.first().and_then(|holder| holder.pct_of_supply);
    let top5_total = holders
        .iter()
        .take(5)
        .map(|holder| parse_decimal_amount(&holder.quantity))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<f64>();
    let top5_pct = Some((top5_total / supply) * 100.0);

    Ok(HolderInfo {
        top1_pct,
        top5_pct,
        top_holders,
    })
}

fn parse_decimal_amount(value: &str) -> Result<f64, ProviderError> {
    value
        .parse::<f64>()
        .map_err(|_| ProviderError::InvalidResponse)
}

fn unknown_holder_info() -> HolderInfo {
    HolderInfo {
        top1_pct: None,
        top5_pct: None,
        top_holders: vec![],
    }
}

fn build_creation_info(created_at_unix: u64) -> Result<CreationInfo, ProviderError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ProviderError::InvalidResponse)?
        .as_secs();

    if created_at_unix > now {
        return Err(ProviderError::InvalidResponse);
    }

    let age_seconds = now - created_at_unix;
    let age_band = if age_seconds < 24 * 60 * 60 {
        AgeBand::LessThan24h
    } else if age_seconds < 7 * 24 * 60 * 60 {
        AgeBand::Day1To7
    } else {
        AgeBand::GreaterThan7d
    };

    Ok(CreationInfo {
        created_at: Some(created_at_unix.to_string()),
        age_seconds: Some(age_seconds),
        age_band,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0x10").unwrap(), 16);
        assert_eq!(parse_hex_u64("ff").unwrap(), 255);
        assert!(parse_hex_u64("0xnot-hex").is_err());
    }

    #[test]
    fn test_parse_hex_u128() {
        assert_eq!(
            parse_hex_u128("0xde0b6b3a7640000").unwrap(),
            1_000_000_000_000_000_000
        );
        assert!(parse_hex_u128("0xnot-hex").is_err());
    }

    #[test]
    fn test_has_contract_code() {
        assert!(!has_contract_code("0x"));
        assert!(!has_contract_code("0x0"));
        assert!(has_contract_code("0x60016000"));
    }

    #[test]
    fn test_build_creation_info_from_unix_time() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let creation = build_creation_info(now - 8 * 24 * 60 * 60).unwrap();

        assert!(creation.created_at.is_some());
        assert!(creation.age_seconds.unwrap() >= 8 * 24 * 60 * 60);
        assert!(matches!(creation.age_band, AgeBand::GreaterThan7d));
    }

    #[test]
    fn test_build_holder_info_from_basescan_holders() {
        let holders: Vec<BasescanTokenHolder> = serde_json::from_value(json!([
            {
                "TokenHolderAddress": "0xholder1",
                "TokenHolderQuantity": "250000"
            },
            {
                "TokenHolderAddress": "0xholder2",
                "TokenHolderQuantity": "150000"
            },
            {
                "TokenHolderAddress": "0xholder3",
                "TokenHolderQuantity": "100000"
            },
            {
                "TokenHolderAddress": "0xholder4",
                "TokenHolderQuantity": "50000"
            },
            {
                "TokenHolderAddress": "0xholder5",
                "TokenHolderQuantity": "50000"
            }
        ]))
        .unwrap();

        let holders = build_holder_info(holders, "1000000", 3).unwrap();

        assert_eq!(holders.top_holders.len(), 3);
        assert_eq!(holders.top_holders[0].address, "0xholder1");
        assert_eq!(holders.top1_pct, Some(25.0));
        assert_eq!(holders.top5_pct, Some(60.0));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_metadata() {
        // USDC on Base
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

        let api_key =
            std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set for this test");

        let provider = AlchemyProvider::new(api_key, "base");

        let metadata = provider.fetch_metadata(usdc_base).await.unwrap();

        println!("\n=== USDC Base Metadata ===");
        println!("{:#?}", metadata);
        assert_eq!(metadata.decimals, Some(6));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_authorities() {
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

        let api_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");

        let provider = AlchemyProvider::new(api_key, "base");

        let authorities = provider.fetch_authorities(usdc_base).await.unwrap();

        println!("\n=== USDC Base Authorities ===");
        println!("{:#?}", authorities);
        // USDC on Base has an owner (Circle)
        assert!(authorities.owner.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_supply() {
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

        let api_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");

        let provider = AlchemyProvider::new(api_key, "base");

        let supply = provider.fetch_supply(usdc_base).await.unwrap();

        println!("\n=== USDC Base Supply ===");
        println!("{:#?}", supply);
        assert!(supply.total_supply.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_creation_time() {
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

        let api_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");

        let provider = AlchemyProvider::new(api_key, "base");

        let creation = provider.fetch_creation_time(usdc_base).await.unwrap();

        println!("\n=== USDC Base Creation Time ===");
        println!("{:#?}", creation);
        assert!(creation.created_at.is_some());
        assert!(creation.age_seconds.is_some());
        assert!(matches!(creation.age_band, AgeBand::GreaterThan7d));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_holders() {
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

        std::env::var("BASESCAN_API_KEY").expect("BASESCAN_API_KEY must be set");
        let api_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");

        let provider = AlchemyProvider::new(api_key, "base");

        let holders = provider.fetch_holders(usdc_base, 10).await.unwrap();

        println!("\n=== USDC Base Holders ===");
        println!("{:#?}", holders);
        assert!(holders.top1_pct.is_some());
        assert!(holders.top5_pct.is_some());
        assert!(!holders.top_holders.is_empty());
    }
}
