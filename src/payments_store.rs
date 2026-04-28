use serde::Serialize;

#[derive(Clone)]
pub struct SupabasePaymentStore {
    url: String,
    service_role_key: String,
}

impl SupabasePaymentStore {
    pub fn new(url: String, service_role_key: String) -> Self {
        Self {
            url: normalize_supabase_url(&url),
            service_role_key: normalize_service_role_key(&service_role_key),
        }
    }

    pub async fn has_used_tx(&self, tx_hash: &str) -> Result<bool, PaymentStoreError> {
        let endpoint = format!(
            "{}/rest/v1/used_payment_txs?tx_hash=eq.{}&select=tx_hash&limit=1",
            self.url,
            encode_query_value(tx_hash)
        );

        let response = self
            .client()
            .get(&endpoint)
            .bearer_auth(&self.service_role_key)
            .header("apikey", &self.service_role_key)
            .send()
            .await
            .map_err(|e| PaymentStoreError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(PaymentStoreError::InvalidResponse { status, body });
        }

        let rows: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| PaymentStoreError::Network(e.to_string()))?;

        Ok(!rows.is_empty())
    }

    pub async fn store_used_tx(&self, record: UsedPaymentTxRecord) -> Result<(), PaymentStoreError> {
        let endpoint = format!("{}/rest/v1/used_payment_txs", self.url);

        let response = self
            .client()
            .post(&endpoint)
            .bearer_auth(&self.service_role_key)
            .header("apikey", &self.service_role_key)
            .header("Prefer", "resolution=ignore-duplicates")
            .json(&record)
            .send()
            .await
            .map_err(|e| PaymentStoreError::Network(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err(PaymentStoreError::InvalidResponse { status, body })
        }
    }

    fn client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct UsedPaymentTxRecord {
    pub tx_hash: String,
    pub report_access_id: String,
    pub token_address: String,
    pub amount_usdc: Option<String>,
}

#[derive(Debug)]
pub enum PaymentStoreError {
    Network(String),
    InvalidResponse { status: u16, body: String },
}

fn encode_query_value(value: &str) -> String {
    value.replace('%', "%25").replace(',', "%2C")
}

fn normalize_supabase_url(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/rest/v1")
        .to_string()
}

fn normalize_service_role_key(value: &str) -> String {
    value
        .trim()
        .strip_prefix("Bearer ")
        .unwrap_or_else(|| value.trim())
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_supabase_url() {
        assert_eq!(
            normalize_supabase_url("https://example.supabase.co"),
            "https://example.supabase.co"
        );
        assert_eq!(
            normalize_supabase_url("https://example.supabase.co/rest/v1/"),
            "https://example.supabase.co"
        );
    }

    #[test]
    fn test_normalize_service_role_key() {
        assert_eq!(normalize_service_role_key(" key\n"), "key");
        assert_eq!(normalize_service_role_key("Bearer key"), "key");
    }
}
