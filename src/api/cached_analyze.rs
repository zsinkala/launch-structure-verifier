use crate::api::types::{AnalyzeRequest, AnalyzeResponse};
use crate::providers::TokenProvider;
use crate::cache::{SimpleCache, simple_cache::ttl_for_response};
use super::analyze::analyze;

pub async fn analyze_with_cache<P: TokenProvider>(
    request: AnalyzeRequest,
    provider: &P,
    cache: &mut SimpleCache,
) -> AnalyzeResponse {
    // Generate cache key
    let cache_key = format!(
        "{}:{}:{}:{}",
        request.chain,
        request.address,
        request.options.include_holders,
        request.options.max_holders
    );

    // Check cache first (unless force_refresh)
    if !request.options.force_refresh {
        if let Some(cached_response) = cache.get(&cache_key) {
            return cached_response;
        }
    }

    // Cache miss or force refresh - fetch fresh data
    let response = analyze(request, provider).await;

    // Determine TTL based on token age
    let ttl = ttl_for_response(&response);

    // Store in cache
    cache.set(cache_key, response.clone(), ttl);

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mocks::MockProvider;
    use crate::types::*;
    use crate::api::types::AnalyzeOptions;

    #[tokio::test]
    async fn test_cache_hit() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("Test".to_string()),
                symbol: Some("TEST".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            supply: Some(SupplyInfo {
                total_supply: Some(1000000.0),
                total_supply_raw: Some("1000000".to_string()),
            }),
            holders: Some(HolderInfo {
                top1_pct: Some(10.0),
                top5_pct: Some(30.0),
                top_holders: vec![],
            }),
            creation: Some(CreationInfo {
                created_at: Some("2026-01-20T00:00:00Z".to_string()),
                age_seconds: Some(864000),
                age_band: AgeBand::GreaterThan7d,
            }),
        };

        let provider = MockProvider::new("test").with_facts("test_token", facts);
        let mut cache = SimpleCache::new();

        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: "test_token".to_string(),
            options: AnalyzeOptions::default(),
        };

        // First call - cache miss
        let response1 = analyze_with_cache(request.clone(), &provider, &mut cache).await;
        let analysis_id1 = response1.analysis_id.clone();

        // Second call - should hit cache
        let response2 = analyze_with_cache(request, &provider, &mut cache).await;
        let analysis_id2 = response2.analysis_id.clone();

        // Should return same analysis (from cache)
        assert_eq!(analysis_id1, analysis_id2);
        assert_eq!(cache.size(), 1);
    }

    #[tokio::test]
    async fn test_force_refresh_bypasses_cache() {
        let facts = TokenFacts {
            metadata: Some(Metadata {
                name: Some("Test".to_string()),
                symbol: Some("TEST".to_string()),
                decimals: Some(9),
                standard: TokenStandard::SplToken,
            }),
            authorities: Some(AuthorityInfo {
                mint_authority: None,
                freeze_authority: None,
                owner: None,
                mint_mutable: Some(false),
            }),
            supply: None,
            holders: None,
            creation: None,
        };

        let provider = MockProvider::new("test").with_facts("test_token", facts);
        let mut cache = SimpleCache::new();

        let request = AnalyzeRequest {
            chain: "solana".to_string(),
            address: "test_token".to_string(),
            options: AnalyzeOptions {
                include_holders: true,
                max_holders: 10,
                force_refresh: false,
            },
        };

        // First call
        let response1 = analyze_with_cache(request.clone(), &provider, &mut cache).await;
        let id1 = response1.analysis_id.clone();

        // Second call with force_refresh
        let request_refresh = AnalyzeRequest {
            options: AnalyzeOptions {
                force_refresh: true,
                ..request.options
            },
            ..request
        };

        let response2 = analyze_with_cache(request_refresh, &provider, &mut cache).await;
        let id2 = response2.analysis_id.clone();

        // Should have different analysis IDs (fresh analysis)
        assert_ne!(id1, id2);
    }
}
