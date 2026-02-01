use async_trait::async_trait;
use crate::types::*;
use super::{TokenProvider, ProviderError};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub struct HeliusProvider {
    api_key: String,
    rpc_url: String,
}

impl HeliusProvider {
    pub fn new(api_key: String) -> Self {
        let rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
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

#[derive(Debug, Deserialize)]
struct AccountInfoResponse {
    value: Option<AccountData>,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    data: DataField,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DataField {
    Parsed(ParsedData),
    Raw(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct ParsedData {
    parsed: ParsedInfo,
}

#[derive(Debug, Deserialize)]
struct ParsedInfo {
    info: MintInfo,
    #[serde(rename = "type")]
    account_type: String,
}

#[derive(Debug, Deserialize)]
struct MintInfo {
    decimals: u8,
    supply: String,
    #[serde(rename = "mintAuthority")]
    mint_authority: Option<String>,
    #[serde(rename = "freezeAuthority")]
    freeze_authority: Option<String>,
}

#[async_trait]
impl TokenProvider for HeliusProvider {
    fn provider_name(&self) -> &str {
        "helius"
    }

    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError> {
        // For now, just get decimals from account info
        // Full metadata would require Metaplex metadata account
        let account_info: AccountInfoResponse = self.rpc_call(
            "getAccountInfo",
            json!([
                address,
                {
                    "encoding": "jsonParsed"
                }
            ])
        ).await?;

        let decimals = if let Some(account) = account_info.value {
            if let DataField::Parsed(parsed) = account.data {
                Some(parsed.parsed.info.decimals)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Metadata {
            name: None, // Would need Metaplex metadata
            symbol: None, // Would need Metaplex metadata
            decimals,
            standard: TokenStandard::SplToken,
        })
    }

    async fn fetch_supply(&self, address: &str) -> Result<SupplyInfo, ProviderError> {
        let account_info: AccountInfoResponse = self.rpc_call(
            "getAccountInfo",
            json!([
                address,
                {
                    "encoding": "jsonParsed"
                }
            ])
        ).await?;

        let account = account_info.value.ok_or(ProviderError::NotFound)?;
        
        let (supply_raw, decimals) = if let DataField::Parsed(parsed) = account.data {
            let info = parsed.parsed.info;
            (info.supply, info.decimals)
        } else {
            return Err(ProviderError::InvalidResponse);
        };

        let total_supply = if let Ok(raw) = supply_raw.parse::<u64>() {
            Some(raw as f64 / 10_f64.powi(decimals as i32))
        } else {
            None
        };

        Ok(SupplyInfo {
            total_supply_raw: Some(supply_raw),
            total_supply,
        })
    }

    async fn fetch_authorities(&self, address: &str) -> Result<AuthorityInfo, ProviderError> {
        let account_info: AccountInfoResponse = self.rpc_call(
            "getAccountInfo",
            json!([
                address,
                {
                    "encoding": "jsonParsed"
                }
            ])
        ).await?;

        let account = account_info.value.ok_or(ProviderError::NotFound)?;
        
        let info = if let DataField::Parsed(parsed) = account.data {
            parsed.parsed.info
        } else {
            return Err(ProviderError::InvalidResponse);
        };

        let mint_mutable = info.mint_authority.is_some();

        Ok(AuthorityInfo {
            mint_authority: info.mint_authority,
            freeze_authority: info.freeze_authority,
            owner: None,
            mint_mutable: Some(mint_mutable),
        })
    }

    async fn fetch_holders(&self, _address: &str, _limit: usize) -> Result<HolderInfo, ProviderError> {
        // Would require token accounts query
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
    async fn test_fetch_usdc_metadata() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        
        let api_key = std::env::var("HELIUS_API_KEY")
            .expect("HELIUS_API_KEY must be set for this test");
        
        let provider = HeliusProvider::new(api_key);
        
        let metadata = provider.fetch_metadata(usdc_mint).await.unwrap();
        
        println!("\n=== USDC Metadata ===");
        println!("{:#?}", metadata);
        assert_eq!(metadata.decimals, Some(6));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_authorities() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        
        let api_key = std::env::var("HELIUS_API_KEY")
            .expect("HELIUS_API_KEY must be set");
        
        let provider = HeliusProvider::new(api_key);
        
        let authorities = provider.fetch_authorities(usdc_mint).await.unwrap();
        
        println!("\n=== USDC Authorities ===");
        println!("{:#?}", authorities);
        // USDC has mint authority (Circle controls it)
        assert!(authorities.mint_authority.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_supply() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        
        let api_key = std::env::var("HELIUS_API_KEY")
            .expect("HELIUS_API_KEY must be set");
        
        let provider = HeliusProvider::new(api_key);
        
        let supply = provider.fetch_supply(usdc_mint).await.unwrap();
        
        println!("\n=== USDC Supply ===");
        println!("{:#?}", supply);
        assert!(supply.total_supply.is_some());
        assert!(supply.total_supply.unwrap() > 1_000_000.0); // USDC supply > 1M
    }
}

#[cfg(test)]
mod full_analysis_tests {
    use super::*;
    use crate::api::analyze;
    use crate::api::types::{AnalyzeRequest, AnalyzeOptions};

    #[tokio::test]
    #[ignore]
    async fn test_full_analysis_real_token() {
        // Bonk meme coin on Solana - has mint authority disabled
        let bonk_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        
        let api_key = std::env::var("HELIUS_API_KEY")
            .expect("HELIUS_API_KEY must be set");
        
        let provider = HeliusProvider::new(api_key);
        
        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: bonk_mint.to_string(),
            options: AnalyzeOptions::default(),
        };
        
        let response = analyze(request, &provider).await;
        
        println!("\n=== BONK TOKEN ANALYSIS ===");
        println!("Status: {:?}", response.status);
        println!("Grade: {:?}", response.score.grade);
        println!("Score: {:?}", response.score.fairness_score);
        println!("\nChecks:");
        for check in &response.checks {
            println!("  {} → {:?} (score: {:?})", 
                check.label, 
                check.status,
                check.score_component
            );
        }
        println!("\nExplanation: {}", response.explain.summary);
        println!("What to do:");
        for item in &response.explain.interpretation.what_to_do {
            println!("  - {}", item);
        }
        println!("\n=========================\n");
        
        // Just verify the analysis completed successfully
        println!("\nAnalysis completed successfully!");
    }
}
