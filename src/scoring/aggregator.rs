use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoreComponent {
    pub id: String,
    pub weight: u8,
    pub component_score: Option<u8>,
    pub weighted_points: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoreResult {
    pub model: String,
    pub fairness_score: Option<u8>,
    pub grade: Grade,
    pub components: Vec<ScoreComponent>,
    pub weights_total: u8,
    pub notes: Vec<String>,
}

pub fn aggregate_score(checks: &[CheckResult]) -> ScoreResult {
    let mut weights_total: u8 = 0;
    let mut points_total: f64 = 0.0;
    let mut components = Vec::new();
    let mut has_critical_failure = false;

    for check in checks {
        let component = match check.score_component {
            Some(score) => {
                weights_total += check.weight;
                let weighted_points = (check.weight as f64) * (score as f64 / 100.0);
                points_total += weighted_points;

                ScoreComponent {
                    id: check.id.clone(),
                    weight: check.weight,
                    component_score: Some(score),
                    weighted_points: Some(weighted_points),
                }
            }
            None => {
                ScoreComponent {
                    id: check.id.clone(),
                    weight: check.weight,
                    component_score: None,
                    weighted_points: None,
                }
            }
        };

        components.push(component);

        if matches!(check.severity, Severity::Critical) && matches!(check.status, CheckStatus::Fail) {
            has_critical_failure = true;
        }
    }

    let fairness_score = if weights_total == 0 {
        None
    } else {
        Some(((points_total / weights_total as f64) * 100.0).round() as u8)
    };

    let grade = if has_critical_failure {
        Grade::Compromised
    } else if let Some(score) = fairness_score {
        grade_from_score(score)
    } else {
        Grade::Compromised
    };

    ScoreResult {
        model: "weighted_sum_v1".to_string(),
        fairness_score,
        grade,
        components,
        weights_total,
        notes: vec![
            "Composite score summarizes structure; individual checks are the source of truth.".to_string(),
        ],
    }
}

fn grade_from_score(score: u8) -> Grade {
    if score >= 80 {
        Grade::Strong
    } else if score >= 60 {
        Grade::Mixed
    } else if score >= 40 {
        Grade::Fragile
    } else {
        Grade::Compromised
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_check(
        id: &str,
        status: CheckStatus,
        severity: Severity,
        weight: u8,
        score_component: Option<u8>,
    ) -> CheckResult {
        CheckResult {
            id: id.to_string(),
            label: id.to_string(),
            category: "test".to_string(),
            status,
            severity,
            value: json!(null),
            evidence: json!({}),
            weight,
            score_component,
        }
    }

    #[test]
    fn test_all_pass_strong_grade() {
        let checks = vec![
            make_check("check1", CheckStatus::Pass, Severity::Critical, 25, Some(100)),
            make_check("check2", CheckStatus::Pass, Severity::High, 20, Some(100)),
            make_check("check3", CheckStatus::Pass, Severity::Medium, 20, Some(100)),
        ];

        let result = aggregate_score(&checks);

        assert_eq!(result.fairness_score, Some(100));
        assert!(matches!(result.grade, Grade::Strong));
        assert_eq!(result.weights_total, 65);
    }

    #[test]
    fn test_critical_override_forces_compromised() {
        let checks = vec![
            make_check("mint_authority", CheckStatus::Fail, Severity::Critical, 25, Some(0)),
            make_check("check2", CheckStatus::Pass, Severity::High, 20, Some(100)),
            make_check("check3", CheckStatus::Pass, Severity::Medium, 20, Some(100)),
            make_check("check4", CheckStatus::Pass, Severity::Low, 10, Some(100)),
        ];

        let result = aggregate_score(&checks);

        let expected_score: u8 = ((0.0f64 * 25.0 + 100.0 * 20.0 + 100.0 * 20.0 + 100.0 * 10.0) / 75.0).round() as u8;
        assert_eq!(result.fairness_score, Some(expected_score));
        assert!(matches!(result.grade, Grade::Compromised));
    }

    #[test]
    fn test_unknown_excludes_weight() {
        let checks = vec![
            make_check("check1", CheckStatus::Pass, Severity::Critical, 25, Some(100)),
            make_check("check2", CheckStatus::Unknown, Severity::High, 20, None),
            make_check("check3", CheckStatus::Pass, Severity::Medium, 20, Some(80)),
        ];

        let result = aggregate_score(&checks);

        assert_eq!(result.weights_total, 45);
        assert_eq!(result.fairness_score, Some(91));
        assert!(matches!(result.grade, Grade::Strong));

        let unknown_component = result.components.iter()
            .find(|c| c.id == "check2")
            .unwrap();
        assert_eq!(unknown_component.weighted_points, None);
    }

    #[test]
    fn test_all_unknown_compromised() {
        let checks = vec![
            make_check("check1", CheckStatus::Unknown, Severity::Critical, 25, None),
            make_check("check2", CheckStatus::Unknown, Severity::High, 20, None),
        ];

        let result = aggregate_score(&checks);

        assert_eq!(result.fairness_score, None);
        assert_eq!(result.weights_total, 0);
        assert!(matches!(result.grade, Grade::Compromised));
    }

    #[test]
    fn test_grade_thresholds() {
        let checks_strong = vec![
            make_check("check1", CheckStatus::Pass, Severity::Medium, 50, Some(80)),
        ];
        let result = aggregate_score(&checks_strong);
        assert!(matches!(result.grade, Grade::Strong));

        let checks_mixed = vec![
            make_check("check1", CheckStatus::Pass, Severity::Medium, 50, Some(70)),
        ];
        let result = aggregate_score(&checks_mixed);
        assert!(matches!(result.grade, Grade::Mixed));

        let checks_fragile = vec![
            make_check("check1", CheckStatus::Pass, Severity::Medium, 50, Some(50)),
        ];
        let result = aggregate_score(&checks_fragile);
        assert!(matches!(result.grade, Grade::Fragile));

        let checks_comp = vec![
            make_check("check1", CheckStatus::Pass, Severity::Medium, 50, Some(30)),
        ];
        let result = aggregate_score(&checks_comp);
        assert!(matches!(result.grade, Grade::Compromised));
    }

    #[test]
    fn test_partial_data_honest_scoring() {
        let checks = vec![
            make_check("mint_authority", CheckStatus::Pass, Severity::Critical, 25, Some(100)),
            make_check("freeze_authority", CheckStatus::Pass, Severity::High, 20, Some(100)),
            make_check("holder_concentration", CheckStatus::Unknown, Severity::Medium, 20, None),
            make_check("token_age", CheckStatus::Pass, Severity::Low, 10, Some(70)),
        ];

        let result = aggregate_score(&checks);

        assert_eq!(result.weights_total, 55);
        assert_eq!(result.fairness_score, Some(95));
        assert!(matches!(result.grade, Grade::Strong));
    }
}
