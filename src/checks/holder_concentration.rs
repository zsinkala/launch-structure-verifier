use crate::types::*;
use serde_json::json;

pub fn check_holder_concentration(facts: &TokenFacts) -> CheckResult {
    let holders = match &facts.holders {
        Some(h) => h,
        None => return unknown_result(),
    };
    
    let (top1_pct, top5_pct) = match (holders.top1_pct, holders.top5_pct) {
        (Some(t1), Some(t5)) => (t1, t5),
        _ => return unknown_result(),
    };
    
    let score1 = score_top1(top1_pct);
    let score5 = score_top5(top5_pct);
    let combined = ((score1 + score5) / 2.0).round() as u8;
    
    let status = if combined >= 50 {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    };
    
    let severity = if combined >= 80 {
        Severity::Low
    } else if combined >= 50 {
        Severity::Medium
    } else {
        Severity::High
    };
    
    CheckResult {
        id: "holder_concentration".to_string(),
        label: "Holder concentration".to_string(),
        category: "distribution".to_string(),
        status,
        severity,
        value: json!({
            "top1_pct": top1_pct,
            "top5_pct": top5_pct,
            "sub_scores": {
                "top1": score1,
                "top5": score5
            }
        }),
        evidence: json!({
            "source": "provider",
            "top1_pct": top1_pct,
            "top5_pct": top5_pct,
            "method": "supply-weighted holder distribution"
        }),
        weight: 20,
        score_component: Some(combined),
    }
}

fn score_top1(pct: f64) -> f64 {
    if pct <= 10.0 {
        100.0
    } else if pct <= 20.0 {
        lerp(pct, 10.0, 20.0, 100.0, 60.0)
    } else if pct <= 40.0 {
        lerp(pct, 20.0, 40.0, 60.0, 25.0)
    } else if pct <= 70.0 {
        lerp(pct, 40.0, 70.0, 25.0, 0.0)
    } else {
        0.0
    }
}

fn score_top5(pct: f64) -> f64 {
    if pct <= 30.0 {
        100.0
    } else if pct <= 50.0 {
        lerp(pct, 30.0, 50.0, 100.0, 60.0)
    } else if pct <= 70.0 {
        lerp(pct, 50.0, 70.0, 60.0, 25.0)
    } else if pct <= 90.0 {
        lerp(pct, 70.0, 90.0, 25.0, 0.0)
    } else {
        0.0
    }
}

fn lerp(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    if x <= x0 {
        return y0;
    }
    if x >= x1 {
        return y1;
    }
    y0 + (x - x0) * (y1 - y0) / (x1 - x0)
}

fn unknown_result() -> CheckResult {
    CheckResult {
        id: "holder_concentration".to_string(),
        label: "Holder concentration".to_string(),
        category: "distribution".to_string(),
        status: CheckStatus::Unknown,
        severity: Severity::Medium,
        value: json!(null),
        evidence: json!({
            "source": "provider",
            "error": "holder data unavailable"
        }),
        weight: 20,
        score_component: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_excellent_distribution() {
        let facts = TokenFacts {
            holders: Some(HolderInfo {
                top1_pct: Some(8.5),
                top5_pct: Some(28.0),
                top_holders: vec![],
            }),
            metadata: None,
            supply: None,
            authorities: None,
            creation: None,
        };
        
        let result = check_holder_concentration(&facts);
        
        assert!(matches!(result.status, CheckStatus::Pass));
        assert!(result.score_component.unwrap() >= 95);
    }
    
    #[test]
    fn test_high_concentration_fragile() {
        let facts = TokenFacts {
            holders: Some(HolderInfo {
                top1_pct: Some(62.0),
                top5_pct: Some(88.0),
                top_holders: vec![],
            }),
            metadata: None,
            supply: None,
            authorities: None,
            creation: None,
        };
        
        let result = check_holder_concentration(&facts);
        
        assert!(matches!(result.status, CheckStatus::Fail));
        assert!(matches!(result.severity, Severity::High));
        assert!(result.score_component.unwrap() < 30);
    }
}
