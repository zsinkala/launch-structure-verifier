use serde::{Deserialize, Serialize};
use crate::types::*;
use crate::scoring::ScoreResult;

#[derive(Clone, Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub chain: String,
    pub address: String,
    #[serde(default)]
    pub options: AnalyzeOptions,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnalyzeOptions {
    #[serde(default = "default_true")]
    pub include_holders: bool,
    #[serde(default = "default_max_holders")]
    pub max_holders: usize,
    #[serde(default)]
    pub force_refresh: bool,
}

fn default_true() -> bool { true }
fn default_max_holders() -> usize { 10 }

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            include_holders: true,
            max_holders: 10,
            force_refresh: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AnalyzeResponse {
    pub schema_version: String,
    pub analysis_id: String,
    pub requested_at: String,
    pub chain: String,
    pub address: String,
    pub status: AnalysisStatus,
    pub token: Option<TokenMetadata>,
    pub checks: Vec<CheckResult>,
    pub score: ScoreResult,
    pub explain: ExplainSection,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisStatus {
    Ok,
    Partial,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct TokenMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub total_supply: Option<f64>,
    pub program_standard: String,
    pub created_at: Option<String>,
    pub age_seconds: Option<u64>,
    pub age_band: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainSection {
    pub summary: String,
    pub method: Vec<String>,
    pub interpretation: InterpretationSection,
}

#[derive(Clone, Debug, Serialize)]
pub struct InterpretationSection {
    pub what_to_do: Vec<String>,
}
