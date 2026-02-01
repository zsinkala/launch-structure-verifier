use async_trait::async_trait;
use crate::types::*;
use super::{TokenProvider, ProviderError};
use std::collections::HashMap;

pub struct MockProvider {
    pub name: String,
    pub facts: HashMap<String, TokenFacts>,
    pub errors: HashMap<String, ProviderError>,
}

impl MockProvider {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            facts: HashMap::new(),
            errors: HashMap::new(),
        }
    }
    
    pub fn with_facts(mut self, address: &str, facts: TokenFacts) -> Self {
        self.facts.insert(address.to_string(), facts);
        self
    }
    
    pub fn with_error(mut self, address: &str, error: ProviderError) -> Self {
        self.errors.insert(address.to_string(), error);
        self
    }
}

#[async_trait]
impl TokenProvider for MockProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }
    
    async fn fetch_metadata(&self, address: &str) -> Result<Metadata, ProviderError> {
        if let Some(_err) = self.errors.get(address) {
            return Err(ProviderError::Timeout);
        }
        
        self.facts.get(address)
            .and_then(|f| f.metadata.clone())
            .ok_or(ProviderError::NotFound)
    }
    
    async fn fetch_supply(&self, address: &str) -> Result<SupplyInfo, ProviderError> {
        if let Some(_err) = self.errors.get(address) {
            return Err(ProviderError::Timeout);
        }
        
        self.facts.get(address)
            .and_then(|f| f.supply.clone())
            .ok_or(ProviderError::NotFound)
    }
    
    async fn fetch_authorities(&self, address: &str) -> Result<AuthorityInfo, ProviderError> {
        if let Some(_err) = self.errors.get(address) {
            return Err(ProviderError::Timeout);
        }
        
        self.facts.get(address)
            .and_then(|f| f.authorities.clone())
            .ok_or(ProviderError::NotFound)
    }
    
    async fn fetch_holders(&self, address: &str, _limit: usize) -> Result<HolderInfo, ProviderError> {
        if let Some(_err) = self.errors.get(address) {
            return Err(ProviderError::Timeout);
        }
        
        self.facts.get(address)
            .and_then(|f| f.holders.clone())
            .ok_or(ProviderError::NotFound)
    }
    
    async fn fetch_creation_time(&self, address: &str) -> Result<CreationInfo, ProviderError> {
        if let Some(_err) = self.errors.get(address) {
            return Err(ProviderError::Timeout);
        }
        
        self.facts.get(address)
            .and_then(|f| f.creation.clone())
            .ok_or(ProviderError::NotFound)
    }
}
