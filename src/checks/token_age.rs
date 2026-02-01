use crate::types::*;
use serde_json::json;

pub fn check_token_age(facts: &TokenFacts) -> CheckResult {
    let creation = match &facts.creation {
        Some(c) => c,
        None => return unknown_result(),
    };
    
    let (score, value) = match creation.age_band {
        AgeBand::GreaterThan7d => (100, "stabilizing"),
        AgeBand::Day1To7 => (70, "early"),
        AgeBand::LessThan24h => (40, "extremely_fragile"),
        AgeBand::Unknown => return unknown_result(),
    };
    
    CheckResult {
        id: "token_age".to_string(),
        label: "Token age".to_string(),
        category: "temporal".to_string(),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        value: json!({
            "age_band": format!("{:?}", creation.age_band),
            "age_seconds": creation.age_seconds,
            "interpretation": value,
        }),
        evidence: json!({
            "source": "provider",
            "created_at": creation.created_at,
            "age_seconds": creation.age_seconds,
        }),
        weight: 10,
        score_component: Some(score),
    }
}

fn unknown_result() -> CheckResult {
    CheckResult {
        id: "token_age".to_string(),
        label: "Token age".to_string(),
        category: "temporal".to_string(),
        status: CheckStatus::Unknown,
        severity: Severity::Low,
        value: json!(null),
        evidence: json!({
            "source": "provider",
            "error": "creation time unavailable"
        }),
        weight: 10,
        score_component: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_age_mature() {
        let facts = TokenFacts {
            creation: Some(CreationInfo {
                created_at: Some("2026-01-20T00:00:00Z".to_string()),
                age_seconds: Some(864000),
                age_band: AgeBand::GreaterThan7d,
            }),
            metadata: None,
            supply: None,
            authorities: None,
            holders: None,
        };
        
        let result = check_token_age(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(100));
    }
    
    #[test]
    fn test_token_age_early() {
        let facts = TokenFacts {
            creation: Some(CreationInfo {
                created_at: Some("2026-01-27T00:00:00Z".to_string()),
                age_seconds: Some(259200),
                age_band: AgeBand::Day1To7,
            }),
            metadata: None,
            supply: None,
            authorities: None,
            holders: None,
        };
        
        let result = check_token_age(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(70));
    }
    
    #[test]
    fn test_token_age_very_new() {
        let facts = TokenFacts {
            creation: Some(CreationInfo {
                created_at: Some("2026-01-31T10:00:00Z".to_string()),
                age_seconds: Some(3600),
                age_band: AgeBand::LessThan24h,
            }),
            metadata: None,
            supply: None,
            authorities: None,
            holders: None,
        };
        
        let result = check_token_age(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.score_component, Some(40));
    }
}
