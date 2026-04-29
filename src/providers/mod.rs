// src/providers/mod.rs

use crate::types::*;
use async_trait::async_trait;

#[derive(Debug)]
pub enum ProviderError {
    Timeout,
    InvalidResponse,
    NetworkError(String),
    NotFound,
}

#[async_trait]
pub trait TokenProvider {
    fn provider_name(&self) -> &str;

    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError>;
    async fn fetch_supply(&self, address: &str) -> Result<SupplyInfo, ProviderError>;
    async fn fetch_authorities(&self, address: &str) -> Result<AuthorityInfo, ProviderError>;
    async fn fetch_holders(&self, address: &str, limit: usize)
        -> Result<HolderInfo, ProviderError>;
    async fn fetch_creation_time(&self, address: &str) -> Result<CreationInfo, ProviderError>;
}

// Module declarations
pub mod alchemy;
pub mod helius;
pub mod mocks;

// Re-export for testing
pub use alchemy::AlchemyProvider;
pub use helius::HeliusProvider;
pub use mocks::MockProvider;
