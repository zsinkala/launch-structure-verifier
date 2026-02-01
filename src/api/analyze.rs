use crate::types::*;
use crate::providers::TokenProvider;
use crate::checks::*;
use crate::scoring::aggregate_score;
use super::types::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Main API handler: orchestrates provider calls, checks, and scoring
pub async fn analyze<P: TokenProvider>(
    request: AnalyzeRequest,
    provider: &P,
) -> AnalyzeResponse {
    let analysis_id = generate_analysis_id();
    let requested_at = current_timestamp();
    let mut errors = Vec::new();

    // Gather facts from provider
    let facts = gather_facts(provider, &request.address, &request.options, &mut errors).await;

    // Determine analysis status
    let status = if errors.is_empty() {
        AnalysisStatus::Ok
    } else if facts.metadata.is_some() || facts.authorities.is_some() {
        AnalysisStatus::Partial
    } else {
        AnalysisStatus::Error
    };

    // Run checks based on chain
    let checks = run_checks(&facts, &request.chain);

    // Aggregate score
    let score = aggregate_score(&checks);

    // Build token metadata
    let token = build_token_metadata(&facts);

    // Generate explanation
    let explain = generate_explanation(&checks, &score);

    AnalyzeResponse {
        schema_version: "1.0.0".to_string(),
        analysis_id,
        requested_at,
        chain: request.chain.clone(),
        address: request.address.clone(),
        status,
        token,
        checks,
        score,
        explain,
        errors,
    }
}

async fn gather_facts<P: TokenProvider>(
    provider: &P,
    address: &str,
    options: &AnalyzeOptions,
    errors: &mut Vec<String>,
) -> TokenFacts {
    let mut facts = TokenFacts {
        metadata: None,
        supply: None,
        authorities: None,
        holders: None,
        creation: None,
    };

    // Fetch metadata
    match provider.fetch_metadata(address).await {
        Ok(metadata) => facts.metadata = Some(metadata),
        Err(e) => errors.push(format!("Failed to fetch metadata: {:?}", e)),
    }

    // Fetch supply
    match provider.fetch_supply(address).await {
        Ok(supply) => facts.supply = Some(supply),
        Err(e) => errors.push(format!("Failed to fetch supply: {:?}", e)),
    }

    // Fetch authorities
    match provider.fetch_authorities(address).await {
        Ok(authorities) => facts.authorities = Some(authorities),
        Err(e) => errors.push(format!("Failed to fetch authorities: {:?}", e)),
    }

    // Fetch holders (conditional)
    if options.include_holders {
        match provider.fetch_holders(address, options.max_holders).await {
            Ok(holders) => facts.holders = Some(holders),
            Err(e) => errors.push(format!("Failed to fetch holders: {:?}", e)),
        }
    }

    // Fetch creation time
    match provider.fetch_creation_time(address).await {
        Ok(creation) => facts.creation = Some(creation),
        Err(e) => errors.push(format!("Failed to fetch creation time: {:?}", e)),
    }

    facts
}

fn run_checks(facts: &TokenFacts, chain: &str) -> Vec<CheckResult> {
    let mut checks = Vec::new();

    match chain {
        "solana" => {
            checks.push(check_mint_authority_disabled(facts));
            checks.push(check_freeze_authority_disabled(facts));
            checks.push(check_holder_concentration(facts));
            checks.push(check_token_age(facts));
            checks.push(check_standard_sanity(facts, chain));
        }
        "base" | "evm" | "ethereum" => {
            checks.push(check_ownership_renounced(facts));
            checks.push(check_holder_concentration(facts));
            checks.push(check_token_age(facts));
            checks.push(check_standard_sanity(facts, chain));
        }
        _ => {
            // Unknown chain - run minimal checks
            checks.push(check_holder_concentration(facts));
            checks.push(check_token_age(facts));
        }
    }

    checks
}

fn build_token_metadata(facts: &TokenFacts) -> Option<TokenMetadata> {
    let metadata = facts.metadata.as_ref()?;
    
    Some(TokenMetadata {
        name: metadata.name.clone(),
        symbol: metadata.symbol.clone(),
        decimals: metadata.decimals,
        total_supply: facts.supply.as_ref().and_then(|s| s.total_supply),
        program_standard: format!("{:?}", metadata.standard),
        created_at: facts.creation.as_ref().and_then(|c| c.created_at.clone()),
        age_seconds: facts.creation.as_ref().and_then(|c| c.age_seconds),
        age_band: facts.creation.as_ref()
            .map(|c| format!("{:?}", c.age_band))
            .unwrap_or_else(|| "Unknown".to_string()),
    })
}

fn generate_explanation(checks: &[CheckResult], score: &crate::scoring::ScoreResult) -> ExplainSection {
    let summary = match score.grade {
        Grade::Strong => "Structure looks sound. No major weaknesses detected.".to_string(),
        Grade::Mixed => "Structure is mostly sound with some areas of concern.".to_string(),
        Grade::Fragile => "Structure shows significant fragility. Proceed with caution.".to_string(),
        Grade::Compromised => "Structure is fundamentally compromised. High risk.".to_string(),
    };

    let method = vec![
        "This tool evaluates structural fairness, not price prediction.".to_string(),
        "Each check is verifiable on-chain and scored transparently.".to_string(),
    ];

    let mut what_to_do = Vec::new();

    // Check for critical failures
    let has_failures = checks.iter().any(|c| matches!(c.status, CheckStatus::Fail));
    
    for check in checks {
        if matches!(check.severity, Severity::Critical) && matches!(check.status, CheckStatus::Fail) {
            if check.id == "mint_authority_disabled" {
                what_to_do.push("Mint authority exists: supply is mutable and can be inflated.".to_string());
            } else if check.id == "ownership_renounced" {
                what_to_do.push("Ownership not renounced: contract parameters can still be changed.".to_string());
            }
        }
    }

    // Check for high severity failures
    for check in checks {
        if matches!(check.severity, Severity::High) && matches!(check.status, CheckStatus::Fail) {
            if check.id == "freeze_authority_disabled" {
                what_to_do.push("Freeze authority exists: token balances can be frozen.".to_string());
            }
        }
    }

    // Check for high concentration
    for check in checks {
        if check.id == "holder_concentration" {
            if let Some(score_comp) = check.score_component {
                if score_comp < 50 {
                    what_to_do.push("High holder concentration increases structural fragility.".to_string());
                }
            }
        }
    }

    // If no specific issues found but also no failures, it's a good launch
    if what_to_do.is_empty() && !has_failures {
        what_to_do.push("All structural checks passed. Token appears fairly launched.".to_string());
    } else if what_to_do.is_empty() && has_failures {
        // Generic message for failures we haven't specifically categorized
        what_to_do.push("Some structural checks failed. Review details above.".to_string());
    }

    ExplainSection {
        summary,
        method,
        interpretation: InterpretationSection { what_to_do },
    }
}

fn generate_analysis_id() -> String {
    // Simple ID generation - in production use UUID
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("analysis_{}", now)
}

fn current_timestamp() -> String {
    // ISO 8601 timestamp - in production use proper datetime library
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("2026-01-31T{:02}:{:02}:{:02}Z", 
        (now / 3600) % 24, 
        (now / 60) % 60, 
        now % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mocks::MockProvider;

    #[tokio::test]
    async fn test_analyze_fair_launch_solana() {
        // Create mock provider with fair launch data
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("FairToken".to_string()),
                symbol: Some("FAIR".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            supply: Some(SupplyInfo {
                total_supply_raw: Some("1000000000000000".to_string()),
                total_supply: Some(1000000.0),
            }),
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            holders: Some(HolderInfo {
                top1_pct: Some(8.5),
                top5_pct: Some(28.0),
                top_holders: vec![],
            }),
            creation: Some(CreationInfo {
                created_at: Some("2026-01-20T00:00:00Z".to_string()),
                age_seconds: Some(864000),
                age_band: AgeBand::GreaterThan7d,
            }),
        };

        let provider = MockProvider::new("test").with_facts("test_address", facts);

        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: "test_address".to_string(),
            options: AnalyzeOptions::default(),
        };

        let response = analyze(request, &provider).await;

        assert_eq!(response.status, AnalysisStatus::Ok);
        assert!(matches!(response.score.grade, Grade::Strong));
        assert!(response.score.fairness_score.unwrap() >= 95);
        assert_eq!(response.errors.len(), 0);
    }

    #[tokio::test]
    async fn test_analyze_mint_authority_exists() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("BadToken".to_string()),
                symbol: Some("BAD".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            supply: Some(SupplyInfo {
                total_supply: Some(1000000.0),
                total_supply_raw: Some("1000000000000000".to_string()),
            }),
            authorities: Some(AuthorityInfo {
                mint_authority: Some("BadAuthority".to_string()),
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(true),
            }),
            holders: Some(HolderInfo {
                top1_pct: Some(5.0),
                top5_pct: Some(20.0),
                top_holders: vec![],
            }),
            creation: Some(CreationInfo {
                created_at: Some("2026-01-20T00:00:00Z".to_string()),
                age_seconds: Some(864000),
                age_band: AgeBand::GreaterThan7d,
            }),
        };

        let provider = MockProvider::new("test").with_facts("bad_token", facts);

        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: "bad_token".to_string(),
            options: AnalyzeOptions::default(),
        };

        let response = analyze(request, &provider).await;

        // Grade must be Compromised due to critical failure
        assert!(matches!(response.score.grade, Grade::Compromised));
        assert!(response.explain.interpretation.what_to_do.iter()
            .any(|s| s.contains("Mint authority exists")));
    }

    #[tokio::test]
    async fn test_analyze_partial_data() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("PartialToken".to_string()),
                symbol: Some("PART".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            supply: None, // Missing supply
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            holders: None, // Missing holders
            creation: None, // Missing creation
        };

        let provider = MockProvider::new("test").with_facts("partial_token", facts);

        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: "partial_token".to_string(),
            options: AnalyzeOptions::default(),
        };

        let response = analyze(request, &provider).await;

        assert_eq!(response.status, AnalysisStatus::Partial);
        assert!(response.errors.len() > 0);
        
        // Some checks should be unknown
        let unknown_count = response.checks.iter()
            .filter(|c| matches!(c.status, CheckStatus::Unknown))
            .count();
        assert!(unknown_count > 0);
    }
}
