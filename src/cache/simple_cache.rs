use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::api::types::AnalyzeResponse;

#[derive(Clone)]
pub struct CacheEntry {
    pub response: AnalyzeResponse,
    pub cached_at: u64,
    pub ttl_seconds: u64,
}

pub struct SimpleCache {
    entries: HashMap<String, CacheEntry>,
}

impl SimpleCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<AnalyzeResponse> {
        if let Some(entry) = self.entries.get(key) {
            let now = current_timestamp();
            let age = now.saturating_sub(entry.cached_at);
            
            if age < entry.ttl_seconds {
                // Still valid
                let mut response = entry.response.clone();
                
                // Update cache metadata in response
                response.requested_at = format!("cached_{}", entry.cached_at);
                
                return Some(response);
            }
        }
        None
    }

    pub fn set(&mut self, key: String, response: AnalyzeResponse, ttl_seconds: u64) {
        let entry = CacheEntry {
            response,
            cached_at: current_timestamp(),
            ttl_seconds,
        };
        
        self.entries.insert(key, entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Remove expired entries
    pub fn cleanup(&mut self) {
        let now = current_timestamp();
        self.entries.retain(|_, entry| {
            let age = now.saturating_sub(entry.cached_at);
            age < entry.ttl_seconds
        });
    }
}

impl Default for SimpleCache {
    fn default() -> Self {
        Self::new()
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Determine TTL based on token age
pub fn ttl_for_response(response: &AnalyzeResponse) -> u64 {
    // Check token age from response
    if let Some(token) = &response.token {
        match token.age_band.as_str() {
            "LessThan24h" => 600,      // 10 minutes for very new tokens
            "Day1To7" => 3600,         // 1 hour for early tokens
            "GreaterThan7d" => 3600,   // 1 hour for mature tokens
            "Unknown" => 1800,         // 30 minutes for unknown age
            _ => 3600,                 // Default 1 hour
        }
    } else {
        1800 // 30 minutes if no token metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{AnalyzeResponse, AnalysisStatus, ExplainSection, InterpretationSection};
    use crate::scoring::ScoreResult;
    use crate::types::Grade;

    fn make_test_response() -> AnalyzeResponse {
        AnalyzeResponse {
            schema_version: "1.0.0".to_string(),
            analysis_id: "test123".to_string(),
            requested_at: "2026-01-31T12:00:00Z".to_string(),
            chain: "solana".to_string(),
            address: "test_address".to_string(),
            status: AnalysisStatus::Ok,
            token: None,
            checks: vec![],
            score: ScoreResult {
                model: "weighted_sum_v1".to_string(),
                fairness_score: Some(100),
                grade: Grade::Strong,
                components: vec![],
                weights_total: 100,
                notes: vec![],
            },
            explain: ExplainSection {
                summary: "Test".to_string(),
                method: vec![],
                interpretation: InterpretationSection {
                    what_to_do: vec![],
                },
            },
            errors: vec![],
        }
    }

    #[test]
    fn test_cache_set_and_get() {
        let mut cache = SimpleCache::new();
        let response = make_test_response();
        
        cache.set("test_key".to_string(), response.clone(), 3600);
        
        let cached = cache.get("test_key");
        assert!(cached.is_some());
        
        let cached_response = cached.unwrap();
        assert_eq!(cached_response.analysis_id, "test123");
    }

    #[test]
    fn test_cache_expiration() {
        let mut cache = SimpleCache::new();
        let response = make_test_response();
        
        // Set with 0 second TTL (immediately expired)
        cache.set("test_key".to_string(), response, 0);
        
        // Should not retrieve expired entry
        let cached = cache.get("test_key");
        assert!(cached.is_none());
    }

    #[test]
    fn test_cache_remove() {
        let mut cache = SimpleCache::new();
        let response = make_test_response();
        
        cache.set("test_key".to_string(), response, 3600);
        assert!(cache.get("test_key").is_some());
        
        let removed = cache.remove("test_key");
        assert!(removed);
        assert!(cache.get("test_key").is_none());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = SimpleCache::new();
        let response = make_test_response();
        
        cache.set("key1".to_string(), response.clone(), 3600);
        cache.set("key2".to_string(), response, 3600);
        
        assert_eq!(cache.size(), 2);
        
        cache.clear();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_cache_cleanup() {
        let mut cache = SimpleCache::new();
        let response = make_test_response();
        
        // Add expired entry
        cache.set("expired".to_string(), response.clone(), 0);
        
        // Add valid entry
        cache.set("valid".to_string(), response, 3600);
        
        assert_eq!(cache.size(), 2);
        
        cache.cleanup();
        
        // Only valid entry should remain
        assert_eq!(cache.size(), 1);
        assert!(cache.get("valid").is_some());
        assert!(cache.get("expired").is_none());
    }
}
