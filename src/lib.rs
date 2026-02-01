// src/lib.rs

pub mod types;
pub mod providers;
pub mod checks;
pub mod scoring;
pub mod api;
pub mod cache;
pub mod server;

// Re-export commonly used types
pub use types::*;
pub use providers::TokenProvider;
pub use scoring::{aggregate_score, ScoreResult};
pub use api::{analyze, AnalyzeRequest, AnalyzeResponse};
pub use cache::SimpleCache;
