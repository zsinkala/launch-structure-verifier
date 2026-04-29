// src/lib.rs

pub mod api;
pub mod cache;
pub mod checks;
pub mod payments_store;
pub mod providers;
pub mod scoring;
pub mod server;
pub mod types;

// Re-export commonly used types
pub use api::{analyze, AnalyzeRequest, AnalyzeResponse};
pub use cache::SimpleCache;
pub use providers::TokenProvider;
pub use scoring::{aggregate_score, ScoreResult};
pub use types::*;
