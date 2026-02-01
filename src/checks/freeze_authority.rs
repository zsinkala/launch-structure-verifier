use crate::types::*;
use serde_json::json;

pub fn check_freeze_authority_disabled(facts: &TokenFacts) -> CheckResult {
    let authorities = match &facts.authorities {
        Some(auth) => auth,
        None => return unknown_result(),
    };
    
    let is_disabled = authorities.freeze_authority.is_none();
    
    CheckResult {
        id: "freeze_authority_disabled".to_string(),
        label: "Freeze authority disabled".to_string(),
        category: "supply_control".to_string(),
        status: if is_disabled { CheckStatus::Pass } else { CheckStatus::Fail },
        severity: Severity::High,
        value: json!(is_disabled),
        evidence: json!({
            "source": "provider",
            "freeze_authority": authorities.freeze_authority,
        }),
        weight: 20,
        score_component: if is_disabled { Some(100) } else { Some(0) },
    }
}

fn unknown_result() -> CheckResult {
    CheckResult {
        id: "freeze_authority_disabled".to_string(),
        label: "Freeze authority disabled".to_string(),
        category: "supply_control".to_string(),
        status: CheckStatus::Unknown,
        severity: Severity::High,
        value: json!(null),
        evidence: json!({
            "source": "provider",
            "error": "authority data unavailable"
        }),
        weight: 20,
        score_component: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_freeze_authority_disabled_pass() {
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
        
        let result = check_freeze_authority_disabled(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(100));
        assert!(matches!(result.severity, Severity::High));
    }
    
    #[test]
    fn test_freeze_authority_exists_fail() {
        let facts = TokenFacts {
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: Some("SomeKey123".to_string()),
                owner: None,
                mint_mutable: Some(false),
            }),
            metadata: None,
            supply: None,
            holders: None,
            creation: None,
        };
        
        let result = check_freeze_authority_disabled(&facts);
        
        assert!(matches!(result.status, CheckStatus::Fail));
        assert_eq!(result.score_component, Some(0));
    }
}
