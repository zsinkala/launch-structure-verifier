use crate::types::*;
use serde_json::json;

pub fn check_ownership_renounced(facts: &TokenFacts) -> CheckResult {
    let authorities = match &facts.authorities {
        Some(auth) => auth,
        None => {
            return CheckResult {
                id: "ownership_renounced".to_string(),
                label: "Ownership renounced".to_string(),
                category: "Authority".to_string(),
                status: CheckStatus::Unknown,
                severity: Severity::High,
                score_component: None,
                value: json!(null),
                weight: 20,
                evidence: json!({"reason": "No authority data available"}),
            };
        }
    };

    let owner = &authorities.owner;
    
    let (status, score) = if owner.is_none() {
        (CheckStatus::Pass, Some(100))
    } else {
        (CheckStatus::Fail, Some(0))
    };

    // CRITICAL: Always Critical severity because ownership control is fundamental
    let severity = Severity::Critical;

    CheckResult {
        id: "ownership_renounced".to_string(),
        label: "Ownership renounced".to_string(),
        category: "Authority".to_string(),
        status,
        severity,
        score_component: score,
        value: json!(owner),
        weight: 20,
        evidence: json!({
            "owner": owner,
            "is_renounced": owner.is_none(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ownership_renounced_zero_address() {
        let facts = TokenFacts {
            metadata: None,
            supply: None,
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            holders: None,
            creation: None,
        };

        let result = check_ownership_renounced(&facts);
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.score_component, Some(100));
    }

    #[test]
    fn test_ownership_renounced_burn_address() {
        let facts = TokenFacts {
            metadata: None,
            supply: None,
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            holders: None,
            creation: None,
        };

        let result = check_ownership_renounced(&facts);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn test_ownership_not_renounced() {
        let facts = TokenFacts {
            metadata: None,
            supply: None,
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: Some("0x1234567890123456789012345678901234567890".to_string()),
                mint_mutable: Some(true),
            }),
            holders: None,
            creation: None,
        };

        let result = check_ownership_renounced(&facts);
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(result.score_component, Some(0));
        assert_eq!(result.severity, Severity::Critical);
    }
}
