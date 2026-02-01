use crate::types::*;
use serde_json::json;

pub fn check_mint_authority_disabled(facts: &TokenFacts) -> CheckResult {
    let authorities = match &facts.authorities {
        Some(auth) => auth,
        None => return unknown_result(),
    };
    
    let is_disabled = authorities.mint_authority.is_none();
    
    CheckResult {
        id: "mint_authority_disabled".to_string(),
        label: "Mint authority disabled".to_string(),
        category: "supply_control".to_string(),
        status: if is_disabled { CheckStatus::Pass } else { CheckStatus::Fail },
        severity: Severity::Critical,
        value: json!(is_disabled),
        evidence: json!({
            "source": "provider",
            "mint_authority": authorities.mint_authority,
        }),
        weight: 25,
        score_component: if is_disabled { Some(100) } else { Some(0) },
    }
}

fn unknown_result() -> CheckResult {
    CheckResult {
        id: "mint_authority_disabled".to_string(),
        label: "Mint authority disabled".to_string(),
        category: "supply_control".to_string(),
        status: CheckStatus::Unknown,
        severity: Severity::Critical,
        value: json!(null),
        evidence: json!({
            "source": "provider",
            "error": "authority data unavailable"
        }),
        weight: 25,
        score_component: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mint_authority_disabled_pass() {
        let facts = TokenFacts {
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            metadata: None,
            supply: None,
            holders: None,
            creation: None,
        };
        
        let result = check_mint_authority_disabled(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(100));
        assert!(matches!(result.severity, Severity::Critical));
    }
    
    #[test]
    fn test_mint_authority_exists_fail() {
        let facts = TokenFacts {
            authorities: Some(AuthorityInfo {
                mint_authority: Some("SomeKey123".to_string()),
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(true),
            }),
            metadata: None,
            supply: None,
            holders: None,
            creation: None,
        };
        
        let result = check_mint_authority_disabled(&facts);
        
        assert!(matches!(result.status, CheckStatus::Fail));
        assert_eq!(result.score_component, Some(0));
        assert!(matches!(result.severity, Severity::Critical));
    }
    
    #[test]
    fn test_mint_authority_unknown() {
        let facts = TokenFacts {
            authorities: None,
            metadata: None,
            supply: None,
            holders: None,
            creation: None,
        };
        
        let result = check_mint_authority_disabled(&facts);
        
        assert!(matches!(result.status, CheckStatus::Unknown));
        assert_eq!(result.score_component, None);
    }
}
