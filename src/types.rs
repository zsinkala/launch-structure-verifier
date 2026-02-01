// src/types.rs

use candid::{CandidType, Deserialize};
use serde::Serialize;

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub standard: TokenStandard,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum TokenStandard {
    SplToken,
    SplToken2022,
    Erc20,
    Unknown,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct SupplyInfo {
    pub total_supply_raw: Option<String>,
    pub total_supply: Option<f64>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AuthorityInfo {
    pub mint_authority: Option<String>,
    pub freeze_authority: Option<String>,
    pub owner: Option<String>,
    pub mint_mutable: Option<bool>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct HolderInfo {
    pub top1_pct: Option<f64>,
    pub top5_pct: Option<f64>,
    pub top_holders: Vec<HolderBalance>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct HolderBalance {
    pub address: String,
    pub balance_raw: String,
    pub balance: Option<f64>,
    pub pct_of_supply: Option<f64>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct CreationInfo {
    pub created_at: Option<String>,
    pub age_seconds: Option<u64>,
    pub age_band: AgeBand,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AgeBand {
    LessThan24h,
    Day1To7,
    GreaterThan7d,
    Unknown,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct TokenFacts {
    pub metadata: Option<Metadata>,
    pub supply: Option<SupplyInfo>,
    pub authorities: Option<AuthorityInfo>,
    pub holders: Option<HolderInfo>,
    pub creation: Option<CreationInfo>,
}

// CheckResult uses serde_json::Value for flexible evidence
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub label: String,
    pub category: String,
    pub status: CheckStatus,
    pub severity: Severity,
    pub value: serde_json::Value,
    pub evidence: serde_json::Value,
    pub weight: u8,
    pub score_component: Option<u8>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum Grade {
    Strong,
    Mixed,
    Fragile,
    Compromised,
}
