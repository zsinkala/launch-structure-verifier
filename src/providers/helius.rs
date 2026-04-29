use super::{ProviderError, TokenProvider};
use crate::types::*;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

pub struct HeliusProvider {
    rpc_url: String,
}

impl HeliusProvider {
    pub fn new(api_key: String) -> Self {
        let rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
        Self { rpc_url }
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

#[derive(Debug, Deserialize)]
struct AssetResponse {
    content: Option<AssetContent>,
    #[serde(rename = "token_info")]
    token_info: Option<AssetTokenInfo>,
}

#[derive(Debug, Deserialize)]
struct AssetContent {
    metadata: Option<AssetMetadata>,
}

#[derive(Debug, Deserialize)]
struct AssetMetadata {
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssetTokenInfo {
    decimals: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct TokenLargestAccountsResponse {
    value: Vec<TokenLargestAccount>,
}

#[derive(Debug, Deserialize)]
struct TokenLargestAccount {
    address: String,
    amount: String,
    decimals: u8,
    #[serde(rename = "uiAmount")]
    ui_amount: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TokenSupplyResponse {
    value: TokenSupplyValue,
}

#[derive(Debug, Deserialize)]
struct TokenSupplyValue {
    amount: String,
    decimals: u8,
}

#[derive(Debug, Deserialize)]
struct SignatureInfo {
    signature: String,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
}

#[async_trait]
impl TokenProvider for HeliusProvider {
    fn provider_name(&self) -> &str {
        "helius"
    }

    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError> {
        let account_info: AccountInfoResponse = self
            .rpc_call(
                "getAccountInfo",
                json!([
                    address,
                    {
                        "encoding": "jsonParsed"
                    }
                ]),
            )
            .await?;

        let decimals = if let Some(account) = account_info.value {
            if let DataField::Parsed(parsed) = account.data {
                Some(parsed.parsed.info.decimals)
            } else {
                None
            }
        } else {
            None
        };

        let asset_metadata = self
            .rpc_call::<AssetResponse>(
                "getAsset",
                json!({
                    "id": address
                }),
            )
            .await
            .ok();

        let (name, symbol, asset_decimals) = asset_metadata
            .as_ref()
            .map(extract_asset_metadata)
            .unwrap_or((None, None, None));

        Ok(Metadata {
            name,
            symbol,
            decimals: decimals.or(asset_decimals),
            standard: TokenStandard::SplToken,
        })
    }

    async fn fetch_supply(&self, address: &str) -> Result<SupplyInfo, ProviderError> {
        let account_info: AccountInfoResponse = self
            .rpc_call(
                "getAccountInfo",
                json!([
                    address,
                    {
                        "encoding": "jsonParsed"
                    }
                ]),
            )
            .await?;

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
        let account_info: AccountInfoResponse = self
            .rpc_call(
                "getAccountInfo",
                json!([
                    address,
                    {
                        "encoding": "jsonParsed"
                    }
                ]),
            )
            .await?;

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

    async fn fetch_holders(
        &self,
        address: &str,
        limit: usize,
    ) -> Result<HolderInfo, ProviderError> {
        let largest_accounts: TokenLargestAccountsResponse = self
            .rpc_call("getTokenLargestAccounts", json!([address]))
            .await?;

        let supply: TokenSupplyResponse = self.rpc_call("getTokenSupply", json!([address])).await?;

        build_holder_info(
            largest_accounts.value,
            &supply.value.amount,
            supply.value.decimals,
            limit,
        )
    }

    async fn fetch_creation_time(&self, address: &str) -> Result<CreationInfo, ProviderError> {
        let oldest_block_time = self.fetch_oldest_signature_block_time(address).await?;
        build_creation_info(oldest_block_time)
    }
}

impl HeliusProvider {
    async fn fetch_oldest_signature_block_time(&self, address: &str) -> Result<i64, ProviderError> {
        let mut before: Option<String> = None;
        let mut oldest_block_time: Option<i64> = None;

        for _ in 0..10 {
            let params = match &before {
                Some(signature) => json!([
                    address,
                    {
                        "limit": 1000,
                        "before": signature
                    }
                ]),
                None => json!([
                    address,
                    {
                        "limit": 1000
                    }
                ]),
            };

            let signatures: Vec<SignatureInfo> =
                self.rpc_call("getSignaturesForAddress", params).await?;

            if signatures.is_empty() {
                break;
            }

            for signature in &signatures {
                if let Some(block_time) = signature.block_time {
                    oldest_block_time = Some(block_time);
                }
            }

            before = signatures
                .last()
                .map(|signature| signature.signature.clone());

            if signatures.len() < 1000 {
                break;
            }
        }

        oldest_block_time.ok_or(ProviderError::NotFound)
    }
}

fn extract_asset_metadata(asset: &AssetResponse) -> (Option<String>, Option<String>, Option<u8>) {
    let metadata = asset
        .content
        .as_ref()
        .and_then(|content| content.metadata.as_ref());

    (
        metadata.and_then(|metadata| clean_metadata_string(metadata.name.as_deref())),
        metadata.and_then(|metadata| clean_metadata_string(metadata.symbol.as_deref())),
        asset
            .token_info
            .as_ref()
            .and_then(|token_info| token_info.decimals),
    )
}

fn clean_metadata_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_holder_info(
    accounts: Vec<TokenLargestAccount>,
    supply_raw: &str,
    supply_decimals: u8,
    limit: usize,
) -> Result<HolderInfo, ProviderError> {
    let supply = parse_raw_amount(supply_raw)?;
    if supply <= 0.0 {
        return Err(ProviderError::InvalidResponse);
    }

    let mut top_holders = Vec::new();
    for account in accounts.iter().take(limit) {
        let balance_raw = parse_raw_amount(&account.amount)?;
        let decimals = account.decimals.max(supply_decimals);
        let balance = account
            .ui_amount
            .or_else(|| Some(balance_raw / 10_f64.powi(decimals as i32)));
        let pct_of_supply = Some((balance_raw / supply) * 100.0);

        top_holders.push(HolderBalance {
            address: account.address.clone(),
            balance_raw: account.amount.clone(),
            balance,
            pct_of_supply,
        });
    }

    let top1_pct = top_holders.first().and_then(|holder| holder.pct_of_supply);

    let top5_total = accounts
        .iter()
        .take(5)
        .map(|account| parse_raw_amount(&account.amount))
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

fn parse_raw_amount(value: &str) -> Result<f64, ProviderError> {
    value
        .parse::<f64>()
        .map_err(|_| ProviderError::InvalidResponse)
}

fn build_creation_info(created_at_unix: i64) -> Result<CreationInfo, ProviderError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ProviderError::InvalidResponse)?
        .as_secs() as i64;

    if created_at_unix > now {
        return Err(ProviderError::InvalidResponse);
    }

    let age_seconds = (now - created_at_unix) as u64;
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

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_metadata() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let api_key =
            std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set for this test");

        let provider = HeliusProvider::new(api_key);

        let metadata = provider.fetch_metadata(usdc_mint).await.unwrap();

        println!("\n=== USDC Metadata ===");
        println!("{:#?}", metadata);
        assert_eq!(metadata.decimals, Some(6));
    }

    #[test]
    fn test_extract_asset_metadata() {
        let asset: AssetResponse = serde_json::from_value(json!({
            "content": {
                "metadata": {
                    "name": "Bonk",
                    "symbol": "BONK"
                }
            },
            "token_info": {
                "decimals": 5
            }
        }))
        .unwrap();

        let (name, symbol, decimals) = extract_asset_metadata(&asset);

        assert_eq!(name.as_deref(), Some("Bonk"));
        assert_eq!(symbol.as_deref(), Some("BONK"));
        assert_eq!(decimals, Some(5));
    }

    #[test]
    fn test_build_holder_info_from_largest_accounts() {
        let accounts: Vec<TokenLargestAccount> = serde_json::from_value(json!([
            {
                "address": "holder1",
                "amount": "250000",
                "decimals": 2,
                "uiAmount": 2500.0
            },
            {
                "address": "holder2",
                "amount": "150000",
                "decimals": 2,
                "uiAmount": 1500.0
            },
            {
                "address": "holder3",
                "amount": "100000",
                "decimals": 2,
                "uiAmount": 1000.0
            },
            {
                "address": "holder4",
                "amount": "50000",
                "decimals": 2,
                "uiAmount": 500.0
            },
            {
                "address": "holder5",
                "amount": "50000",
                "decimals": 2,
                "uiAmount": 500.0
            }
        ]))
        .unwrap();

        let holders = build_holder_info(accounts, "1000000", 2, 3).unwrap();

        assert_eq!(holders.top_holders.len(), 3);
        assert_eq!(holders.top_holders[0].address, "holder1");
        assert_eq!(holders.top_holders[0].balance, Some(2500.0));
        assert_eq!(holders.top1_pct, Some(25.0));
        assert_eq!(holders.top5_pct, Some(60.0));
    }

    #[test]
    fn test_build_creation_info_from_unix_time() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let creation = build_creation_info(now - 8 * 24 * 60 * 60).unwrap();

        assert!(creation.created_at.is_some());
        assert!(creation.age_seconds.unwrap() >= 8 * 24 * 60 * 60);
        assert!(matches!(creation.age_band, AgeBand::GreaterThan7d));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_authorities() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set");

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

        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set");

        let provider = HeliusProvider::new(api_key);

        let supply = provider.fetch_supply(usdc_mint).await.unwrap();

        println!("\n=== USDC Supply ===");
        println!("{:#?}", supply);
        assert!(supply.total_supply.is_some());
        assert!(supply.total_supply.unwrap() > 1_000_000.0); // USDC supply > 1M
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_holders() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set");

        let provider = HeliusProvider::new(api_key);

        let holders = provider.fetch_holders(usdc_mint, 10).await.unwrap();

        println!("\n=== USDC Holders ===");
        println!("{:#?}", holders);
        assert!(holders.top1_pct.is_some());
        assert!(holders.top5_pct.is_some());
        assert!(!holders.top_holders.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_usdc_creation_time() {
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set");

        let provider = HeliusProvider::new(api_key);

        let creation = provider.fetch_creation_time(usdc_mint).await.unwrap();

        println!("\n=== USDC Creation Time ===");
        println!("{:#?}", creation);
        assert!(creation.created_at.is_some());
        assert!(creation.age_seconds.is_some());
        assert!(matches!(creation.age_band, AgeBand::GreaterThan7d));
    }
}

#[cfg(test)]
mod full_analysis_tests {
    use super::*;
    use crate::api::analyze;
    use crate::api::types::{AnalyzeOptions, AnalyzeRequest};

    #[tokio::test]
    #[ignore]
    async fn test_full_analysis_real_token() {
        // Bonk meme coin on Solana - has mint authority disabled
        let bonk_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set");

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
            println!(
                "  {} → {:?} (score: {:?})",
                check.label, check.status, check.score_component
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
