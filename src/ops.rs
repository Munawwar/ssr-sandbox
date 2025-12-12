//! Shared ops module - used by both build.rs (snapshot) and runtime.rs
//!
//! This module contains all custom ops and the extension! macro definition.
//! It must be importable by both the main crate and the build script.

use deno_core::{op2, OpState};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================================
// Console Output Capture
// ============================================================================

/// Captured console output from the sandboxed runtime
#[derive(Debug, Default, Clone)]
pub struct ConsoleOutput {
    pub logs: Vec<String>,
    pub warns: Vec<String>,
    pub errors: Vec<String>,
}

#[op2(fast)]
pub fn op_console_log(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.logs.push(msg.to_string());
    }
}

#[op2(fast)]
pub fn op_console_warn(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.warns.push(msg.to_string());
    }
}

#[op2(fast)]
pub fn op_console_error(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.errors.push(msg.to_string());
    }
}

// ============================================================================
// Fetch API
// ============================================================================

/// Configuration for fetch allowlist
#[derive(Debug, Clone, Default)]
pub struct FetchConfig {
    pub allowed_origins: Vec<String>,
}

impl FetchConfig {
    pub fn is_origin_allowed(&self, url: &url::Url) -> bool {
        if self.allowed_origins.is_empty() {
            return false;
        }
        let origin = url.origin().ascii_serialization();
        self.allowed_origins.iter().any(|allowed| origin == *allowed)
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

    // Delegate to the actual implementation (can be called recursively for redirects)
    do_fetch(request, config).await
}

/// Internal fetch implementation (can be called recursively for redirects)
async fn do_fetch(
    request: FetchRequest,
    config: FetchConfig,
) -> Result<FetchResponse, deno_core::error::AnyError> {
    use anyhow::anyhow;
    use reqwest::{Client, Method};
    use url::Url;

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

    if let Some(ref headers) = request.headers {
        for (key, value) in headers {
            req_builder = req_builder.header(key, value);
        }
    }

    if let Some(body) = request.body {
        req_builder = req_builder.body(body);
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| anyhow!("Fetch failed: {}", e))?;

    let status = response.status();
    let final_url = response.url().clone();

    // Handle redirects - only allow same-origin
    if status.is_redirection() {
        if let Some(location) = response.headers().get("location") {
            let location_str = location.to_str().map_err(|_| anyhow!("Invalid redirect location"))?;
            let redirect_url = final_url.join(location_str)
                .map_err(|e| anyhow!("Invalid redirect URL: {}", e))?;

            if redirect_url.origin() != url.origin() {
                return Err(anyhow!(
                    "Fetch blocked: redirect to different origin '{}' (original: '{}')",
                    redirect_url.origin().ascii_serialization(),
                    url.origin().ascii_serialization()
                ).into());
            }

            if !config.is_origin_allowed(&redirect_url) {
                return Err(anyhow!(
                    "Fetch blocked: redirect origin '{}' is not in the allowlist",
                    redirect_url.origin().ascii_serialization()
                ).into());
            }

            // Follow redirect with a recursive call via Box::pin
            let redirect_request = FetchRequest {
                url: redirect_url.to_string(),
                method: Some("GET".to_string()),
                headers: request.headers.clone(),
                body: None,
            };

            return Box::pin(do_fetch(redirect_request, config)).await;
        }
    }

    let mut resp_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            resp_headers.insert(key.to_string(), v.to_string());
        }
    }

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

// ============================================================================
// Extension Definition
// ============================================================================

deno_core::extension!(
    ssr_runtime,
    ops = [
        op_console_log,
        op_console_warn,
        op_console_error,
        op_fetch,
    ],
    esm_entry_point = "ext:ssr_runtime/bootstrap.js",
    esm = ["ext:ssr_runtime/bootstrap.js" = "src/bootstrap.js"],
);

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
        assert!(config.is_origin_allowed(&url::Url::parse("https://api.example.com/users").unwrap()));
        assert!(config.is_origin_allowed(&url::Url::parse("https://api.example.com/").unwrap()));
        assert!(config.is_origin_allowed(&url::Url::parse("http://localhost:3000/api").unwrap()));

        // Not allowed
        assert!(!config.is_origin_allowed(&url::Url::parse("https://evil.com/api").unwrap()));
        assert!(!config.is_origin_allowed(&url::Url::parse("http://api.example.com/users").unwrap())); // http vs https
        assert!(!config.is_origin_allowed(&url::Url::parse("https://api.example.com:8080/").unwrap())); // different port
    }

    #[test]
    fn test_empty_allowlist() {
        let config = FetchConfig {
            allowed_origins: vec![],
        };

        assert!(!config.is_origin_allowed(&url::Url::parse("https://anything.com").unwrap()));
    }
}
