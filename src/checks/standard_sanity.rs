use crate::types::*;
use serde_json::json;

pub fn check_standard_sanity(facts: &TokenFacts, chain: &str) -> CheckResult {
    let metadata = match &facts.metadata {
        Some(m) => m,
        None => return unknown_result(),
    };
    
    let (is_standard, severity) = match chain {
        "solana" => check_solana_standard(&metadata.standard),
        "base" | "evm" => check_evm_standard(&metadata.standard, &metadata.decimals),
        _ => (false, Severity::Medium),
    };
    
    CheckResult {
        id: "standard_sanity".to_string(),
        label: "Standard sanity".to_string(),
        category: "interface".to_string(),
        status: if is_standard { CheckStatus::Pass } else { CheckStatus::Fail },
        severity,
        value: json!({
            "standard": format!("{:?}", metadata.standard),
            "chain": chain,
        }),
        evidence: json!({
            "source": "provider",
            "standard": format!("{:?}", metadata.standard),
            "decimals": metadata.decimals,
        }),
        weight: 10,
        score_component: if is_standard { Some(100) } else { Some(0) },
    }
}

fn check_solana_standard(standard: &TokenStandard) -> (bool, Severity) {
    match standard {
        TokenStandard::SplToken | TokenStandard::SplToken2022 => (true, Severity::Medium),
        TokenStandard::Unknown => (false, Severity::High),
        _ => (false, Severity::Medium),
    }
}

fn check_evm_standard(standard: &TokenStandard, decimals: &Option<u8>) -> (bool, Severity) {
    match standard {
        TokenStandard::Erc20 if decimals.is_some() => (true, Severity::Medium),
        TokenStandard::Unknown => (false, Severity::High),
        _ => (false, Severity::Medium),
    }
}

fn unknown_result() -> CheckResult {
    CheckResult {
        id: "standard_sanity".to_string(),
        label: "Standard sanity".to_string(),
        category: "interface".to_string(),
        status: CheckStatus::Unknown,
        severity: Severity::Medium,
        value: json!(null),
        evidence: json!({
            "source": "provider",
            "error": "metadata unavailable"
        }),
        weight: 10,
        score_component: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_solana_spl_token_pass() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("Test".to_string()),
                symbol: Some("TEST".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            supply: None,
            authorities: None,
            holders: None,
            creation: None,
        };
        
        let result = check_standard_sanity(&facts, "solana");
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(100));
    }
    
    #[test]
    fn test_evm_erc20_pass() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("Test".to_string()),
                symbol: Some("TEST".to_string()),
                decimals: Some(18),
                standard: TokenStandard::Erc20,
            }),
            supply: None,
            authorities: None,
            holders: None,
            creation: None,
        };
        
        let result = check_standard_sanity(&facts, "evm");
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(100));
    }
    
    #[test]
    fn test_unknown_standard_fail() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("Test".to_string()),
                symbol: Some("TEST".to_string()),
                decimals: None,
                standard: TokenStandard::Unknown,
            }),
            supply: None,
            authorities: None,
            holders: None,
            creation: None,
        };
        
        let result = check_standard_sanity(&facts, "solana");
        
        assert!(matches!(result.status, CheckStatus::Fail));
        assert_eq!(result.score_component, Some(0));
        assert!(matches!(result.severity, Severity::High));
    }
}
