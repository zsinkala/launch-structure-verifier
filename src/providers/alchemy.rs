use async_trait::async_trait;
use crate::types::*;
use super::{TokenProvider, ProviderError};
use serde::Deserialize;
use serde_json::json;

pub struct AlchemyProvider {
    api_key: String,
    rpc_url: String,
}

impl AlchemyProvider {
    pub fn new(api_key: String, chain: &str) -> Self {
        let rpc_url = match chain {
            "base" => format!("https://base-mainnet.g.alchemy.com/v2/{}", api_key),
            "ethereum" => format!("https://eth-mainnet.g.alchemy.com/v2/{}", api_key),
            _ => format!("https://base-mainnet.g.alchemy.com/v2/{}", api_key),
        };
        
        Self {
            api_key,
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

        let text = response.text().await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        
        eprintln!("RPC Response: {}", text);
        
        let rpc_response: RpcResponse<T> = serde_json::from_str(&text)
            .map_err(|e| {
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

#[async_trait]
impl TokenProvider for AlchemyProvider {
    fn provider_name(&self) -> &str {
        "alchemy"
    }

    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError> {
        // ERC20 decimals() function signature: 0x313ce567
        let decimals_data = "0x313ce567";

        // Call decimals()
        let decimals_result: String = self.rpc_call(
            "eth_call",
            json!([
                {
                    "to": address,
                    "data": decimals_data
                },
                "latest"
            ])
        ).await?;

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

        let supply_hex: String = self.rpc_call(
            "eth_call",
            json!([
                {
                    "to": address,
                    "data": total_supply_data
                },
                "latest"
            ])
        ).await?;

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

        let owner_result: String = self.rpc_call(
            "eth_call",
            json!([
                {
                    "to": address,
                    "data": owner_data
                },
                "latest"
            ])
        ).await.unwrap_or_else(|_| "0x".to_string());

        // Extract address from result (last 40 chars)
        let owner = if owner_result.len() >= 42 {
            let addr = format!("0x{}", &owner_result[owner_result.len()-40..]);
            
            // Check if owner is zero address or burn address
            if addr == "0x0000000000000000000000000000000000000000" 
               || addr == "0x000000000000000000000000000000000000dead" {
                None
            } else {
                Some(addr)
            }
        } else {
            None
        };

        let mint_mutable = owner.is_some();

        Ok(AuthorityInfo {
            mint_authority: None, // EVM doesn't use this concept
            freeze_authority: None, // EVM doesn't use this concept
            owner,
            mint_mutable: Some(mint_mutable),
        })
    }

    async fn fetch_holders(&self, _address: &str, _limit: usize) -> Result<HolderInfo, ProviderError> {
        // Would require Alchemy's token holder API
        Ok(HolderInfo {
            top1_pct: None,
            top5_pct: None,
            top_holders: vec![],
        })
    }

    async fn fetch_creation_time(&self, _address: &str) -> Result<CreationInfo, ProviderError> {
        // Would require transaction history
        Ok(CreationInfo {
            created_at: None,
            age_seconds: None,
            age_band: AgeBand::Unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_base_metadata() {
        // USDC on Base
        let usdc_base = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
        
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .expect("ALCHEMY_API_KEY must be set for this test");
        
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
        
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .expect("ALCHEMY_API_KEY must be set");
        
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
        
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .expect("ALCHEMY_API_KEY must be set");
        
        let provider = AlchemyProvider::new(api_key, "base");
        
        let supply = provider.fetch_supply(usdc_base).await.unwrap();
        
        println!("\n=== USDC Base Supply ===");
        println!("{:#?}", supply);
        assert!(supply.total_supply.is_some());
    }
}
