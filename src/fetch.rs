//! Sandboxed fetch implementation with origin allowlist.
//!
//! Security model:
//! - Only URLs matching allowed origins can be fetched
//! - Redirects only followed if they stay within the same origin
//! - Integrates with the overall render timeout

use anyhow::anyhow;
use deno_core::{op2, OpState};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use url::Url;

/// Configuration for fetch allowlist
#[derive(Debug, Clone, Default)]
pub struct FetchConfig {
    /// Allowed origins (e.g., "https://api.example.com")
    /// An origin is scheme + host + port
    pub allowed_origins: Vec<String>,
}

impl FetchConfig {
    pub fn is_origin_allowed(&self, url: &Url) -> bool {
        if self.allowed_origins.is_empty() {
            return false;
        }
        let origin = url.origin().ascii_serialization();
        self.allowed_origins.iter().any(|allowed| {
            // Exact origin match
            origin == *allowed
        })
    }
}

/// Request info passed from JS
#[derive(Debug, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub body: Option<String>,
}

/// Response info returned to JS
#[derive(Debug, Serialize)]
pub struct FetchResponse {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub url: String,
    pub body: String,
}

/// The fetch operation - validates origin and makes the request
#[op2(async)]
#[serde]
pub async fn op_fetch(
    state: Rc<RefCell<OpState>>,
    #[serde] request: FetchRequest,
) -> Result<FetchResponse, deno_core::error::AnyError> {
    // Get config from state
    let config = {
        let state_ref = state.borrow();
        state_ref.borrow::<FetchConfig>().clone()
    };

    // Delegate to the actual implementation
    do_fetch(request, config).await
}

/// Internal fetch implementation (can be called recursively for redirects)
async fn do_fetch(
    request: FetchRequest,
    config: FetchConfig,
) -> Result<FetchResponse, deno_core::error::AnyError> {
    // Parse and validate URL
    let url = Url::parse(&request.url)
        .map_err(|e| anyhow!("Invalid URL '{}': {}", request.url, e))?;

    if !config.is_origin_allowed(&url) {
        return Err(anyhow!(
            "Fetch blocked: origin '{}' is not in the allowlist. Allowed: {:?}",
            url.origin().ascii_serialization(),
            config.allowed_origins
        ).into());
    }

    // Build the request
    let client = Client::builder()
        // Don't follow redirects automatically - we'll handle them manually
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

    let method = match request.method.as_deref().unwrap_or("GET").to_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        other => return Err(anyhow!("Unsupported HTTP method: {}", other).into()),
    };

    let mut req_builder = client.request(method, url.clone());

    // Add headers
    if let Some(ref headers) = request.headers {
        for (key, value) in headers {
            req_builder = req_builder.header(key, value);
        }
    }

    // Add body
    if let Some(body) = request.body {
        req_builder = req_builder.body(body);
    }

    // Make the request
    let response = req_builder
        .send()
        .await
        .map_err(|e| anyhow!("Fetch failed: {}", e))?;

    let status = response.status();
    let final_url = response.url().clone();

    // Handle redirects manually - only allow same-origin
    if status.is_redirection() {
        if let Some(location) = response.headers().get("location") {
            let location_str = location.to_str().map_err(|_| anyhow!("Invalid redirect location"))?;
            let redirect_url = final_url.join(location_str)
                .map_err(|e| anyhow!("Invalid redirect URL: {}", e))?;

            // Check if redirect is to same origin
            if redirect_url.origin() != url.origin() {
                return Err(anyhow!(
                    "Fetch blocked: redirect to different origin '{}' (original: '{}')",
                    redirect_url.origin().ascii_serialization(),
                    url.origin().ascii_serialization()
                ).into());
            }

            // Check if redirect origin is still allowed
            if !config.is_origin_allowed(&redirect_url) {
                return Err(anyhow!(
                    "Fetch blocked: redirect origin '{}' is not in the allowlist",
                    redirect_url.origin().ascii_serialization()
                ).into());
            }

            // Follow the redirect recursively
            let redirect_request = FetchRequest {
                url: redirect_url.to_string(),
                method: Some("GET".to_string()), // Redirects typically become GET
                headers: request.headers.clone(),
                body: None, // Don't send body on redirect
            };

            return Box::pin(do_fetch(redirect_request, config)).await;
        }
    }

    // Collect response headers
    let mut resp_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            resp_headers.insert(key.to_string(), v.to_string());
        }
    }

    // Read body as text
    let body = response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

    Ok(FetchResponse {
        ok: status.is_success(),
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("Unknown").to_string(),
        headers: resp_headers,
        url: final_url.to_string(),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_matching() {
        let config = FetchConfig {
            allowed_origins: vec![
                "https://api.example.com".to_string(),
                "http://localhost:3000".to_string(),
            ],
        };

        // Allowed
        assert!(config.is_origin_allowed(&Url::parse("https://api.example.com/users").unwrap()));
        assert!(config.is_origin_allowed(&Url::parse("https://api.example.com/").unwrap()));
        assert!(config.is_origin_allowed(&Url::parse("http://localhost:3000/api").unwrap()));

        // Not allowed
        assert!(!config.is_origin_allowed(&Url::parse("https://evil.com/api").unwrap()));
        assert!(!config.is_origin_allowed(&Url::parse("http://api.example.com/users").unwrap())); // http vs https
        assert!(!config.is_origin_allowed(&Url::parse("https://api.example.com:8080/").unwrap())); // different port
    }

    #[test]
    fn test_empty_allowlist() {
        let config = FetchConfig {
            allowed_origins: vec![],
        };

        assert!(!config.is_origin_allowed(&Url::parse("https://anything.com").unwrap()));
    }
}
