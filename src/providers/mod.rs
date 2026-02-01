// src/providers/mod.rs

use async_trait::async_trait;
use crate::types::*;

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
    async fn fetch_holders(&self, address: &str, limit: usize) -> Result<HolderInfo, ProviderError>;
    async fn fetch_creation_time(&self, address: &str) -> Result<CreationInfo, ProviderError>;
}

// Module declarations
pub mod mocks;
pub mod helius;
pub mod alchemy;

// Re-export for testing
pub use mocks::MockProvider;
pub use helius::HeliusProvider;
pub use alchemy::AlchemyProvider;
