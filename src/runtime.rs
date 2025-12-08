//! SSR Runtime - executes JavaScript in a sandboxed V8 isolate.
//!
//! Provides only the minimal APIs needed for SSR:
//! - console.log/warn/error (captured, not printed)
//! - URL, URLSearchParams
//! - TextEncoder, TextDecoder
//! - atob, btoa
//! - crypto.randomUUID, crypto.getRandomValues, crypto.subtle.digest
//! - Module loading from allowed directory only
//! - No fs, net, env, or other system access

use crate::fetch::{op_fetch, FetchConfig};
use crate::loader::SandboxedLoader;
use anyhow::{anyhow, Error};
use deno_core::{op2, JsRuntime, ModuleSpecifier, OpState, PollEventLoopOptions, RuntimeOptions};
use std::path::Path;
use std::rc::Rc;

/// Captured console output from the sandboxed runtime
#[derive(Debug, Default, Clone)]
pub struct ConsoleOutput {
    pub logs: Vec<String>,
    pub warns: Vec<String>,
    pub errors: Vec<String>,
}

/// Result of an SSR render
#[derive(Debug)]
pub struct SsrResult {
    pub html: String,
    pub console: ConsoleOutput,
}

// ============================================================================
// Console Ops
// ============================================================================

#[op2(fast)]
fn op_console_log(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.logs.push(msg.to_string());
    }
}

#[op2(fast)]
fn op_console_warn(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.warns.push(msg.to_string());
    }
}

#[op2(fast)]
fn op_console_error(state: &mut OpState, #[string] msg: &str) {
    if let Some(output) = state.try_borrow_mut::<ConsoleOutput>() {
        output.errors.push(msg.to_string());
    }
}

// ============================================================================
// Crypto Ops
// ============================================================================

#[op2]
#[string]
fn op_crypto_random_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[op2(fast)]
fn op_crypto_get_random_values(#[buffer] buf: &mut [u8]) {
    use rand::RngCore;
    rand::thread_rng().fill_bytes(buf);
}

#[op2]
#[buffer]
fn op_crypto_subtle_digest(#[string] algorithm: &str, #[buffer] data: &[u8]) -> Result<Vec<u8>, Error> {
    use sha2::{Sha256, Sha384, Sha512, Digest};

    let result = match algorithm.to_uppercase().replace("-", "").as_str() {
        "SHA256" => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        "SHA384" => {
            let mut hasher = Sha384::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        "SHA512" => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        _ => return Err(anyhow!("Unsupported algorithm: {}. Supported: SHA-256, SHA-384, SHA-512", algorithm)),
    };

    Ok(result)
}

// ============================================================================
// Encoding Ops
// ============================================================================

#[op2]
#[string]
fn op_btoa(#[string] data: &str) -> Result<String, Error> {
    use base64::Engine;
    // btoa expects Latin-1, but we'll be lenient and accept UTF-8
    Ok(base64::engine::general_purpose::STANDARD.encode(data.as_bytes()))
}

#[op2]
#[string]
fn op_atob(#[string] data: &str) -> Result<String, Error> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| anyhow!("Invalid base64: {}", e))?;
    String::from_utf8(bytes).map_err(|e| anyhow!("Invalid UTF-8 in decoded data: {}", e))
}

deno_core::extension!(
    ssr_runtime,
    ops = [
        op_console_log,
        op_console_warn,
        op_console_error,
        op_crypto_random_uuid,
        op_crypto_get_random_values,
        op_crypto_subtle_digest,
        op_btoa,
        op_atob,
        op_fetch,
    ],
    esm_entry_point = "ext:ssr_runtime/bootstrap.js",
    esm = ["ext:ssr_runtime/bootstrap.js" = "src/bootstrap.js"],
);

/// Configuration for the SSR sandbox
pub struct SandboxConfig {
    /// Directory containing the JS chunks (only this dir is accessible)
    pub chunks_dir: String,
    /// Maximum heap size in bytes (default: 64MB, None = unlimited)
    pub max_heap_size: Option<usize>,
    /// Maximum time for a single render in milliseconds (default: 30000ms, None = unlimited)
    pub timeout_ms: Option<u64>,
    /// Allowed origins for fetch() (empty = fetch disabled)
    pub allowed_origins: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            chunks_dir: String::from("./chunks"),
            max_heap_size: Some(64 * 1024 * 1024), // 64MB default
            timeout_ms: Some(30_000), // 30 seconds default
            allowed_origins: vec![], // fetch disabled by default
        }
    }
}

/// Create a sandboxed JS runtime for SSR
pub fn create_runtime(config: &SandboxConfig) -> Result<JsRuntime, Error> {
    let loader = SandboxedLoader::new(&config.chunks_dir)?;

    // Configure V8 heap limits if specified
    let create_params = config.max_heap_size.map(|max_bytes| {
        deno_core::v8::Isolate::create_params().heap_limits(0, max_bytes)
    });

    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(loader)),
        extensions: vec![ssr_runtime::init_ops_and_esm()],
        create_params,
        ..Default::default()
    });

    // Add near-heap-limit callback to gracefully handle OOM
    if config.max_heap_size.is_some() {
        runtime.add_near_heap_limit_callback(|current, initial| {
            // Don't increase the limit - let V8 terminate gracefully
            // Return current limit to trigger OOM error instead of crash
            eprintln!(
                "[ssr-sandbox] Near heap limit: current={}MB, initial={}MB",
                current / (1024 * 1024),
                initial / (1024 * 1024)
            );
            current
        });
    }

    // Initialize console output capture in state
    runtime.op_state().borrow_mut().put(ConsoleOutput::default());

    // Initialize fetch config
    runtime.op_state().borrow_mut().put(FetchConfig {
        allowed_origins: config.allowed_origins.clone(),
    });

    Ok(runtime)
}

/// Execute SSR render and return HTML result
///
/// # Arguments
/// * `runtime` - The sandboxed runtime
/// * `entry_point` - Path to the entry JS file (must be within chunks_dir)
/// * `props` - JSON props to pass to the render function
/// * `timeout_ms` - Optional timeout in milliseconds (None = no timeout)
///
/// # Expected JS module format
/// The entry module should export a default function or a `render` function:
/// ```js
/// export default function render(props) {
///   return "<html>...</html>";
/// }
/// // or
/// export function render(props) {
///   return "<html>...</html>";
/// }
/// ```
pub async fn execute_ssr(
    runtime: &mut JsRuntime,
    entry_point: &Path,
    props: serde_json::Value,
    timeout_ms: Option<u64>,
) -> Result<SsrResult, Error> {
    match timeout_ms {
        Some(ms) => {
            // Get a handle to terminate execution if needed
            let isolate_handle = runtime.v8_isolate().thread_safe_handle();

            // Spawn a task that will terminate execution after timeout
            let timeout_handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                isolate_handle.terminate_execution();
            });

            let result = execute_ssr_inner(runtime, entry_point, props).await;

            // Cancel the timeout task if we finished in time
            timeout_handle.abort();

            // Check if we were terminated due to timeout
            // V8 termination can manifest as various errors
            match &result {
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("terminated")
                        || err_str.contains("unresolved promise")
                        || err_str.contains("Uncaught Error: execution terminated")
                    {
                        Err(anyhow!("Render timed out after {}ms", ms))
                    } else {
                        result
                    }
                }
                _ => result,
            }
        }
        None => execute_ssr_inner(runtime, entry_point, props).await,
    }
}

async fn execute_ssr_inner(
    runtime: &mut JsRuntime,
    entry_point: &Path,
    props: serde_json::Value,
) -> Result<SsrResult, Error> {
    let entry_path = entry_point
        .canonicalize()
        .map_err(|e| anyhow!("Invalid entry point '{}': {}", entry_point.display(), e))?;

    let module_specifier = ModuleSpecifier::from_file_path(&entry_path)
        .map_err(|_| anyhow!("Failed to create module specifier"))?;

    // Call the internal render function (defined in bootstrap.js with closure-protected cache)
    let props_json = serde_json::to_string(&props)?;
    let render_code = format!(
        r#"globalThis.__ssr_internal_render__("{}", {})"#,
        module_specifier, props_json
    );

    let html_global = runtime.execute_script("<ssr>", render_code)?;

    // Run event loop to handle any promises/dynamic imports
    runtime
        .run_event_loop(PollEventLoopOptions::default())
        .await?;

    // Resolve the promise to get the HTML string
    let html_string = {
        let scope = &mut runtime.handle_scope();
        let local = deno_core::v8::Local::new(scope, &html_global);

        if let Some(promise) = deno_core::v8::Local::<deno_core::v8::Promise>::try_from(local).ok()
        {
            match promise.state() {
                deno_core::v8::PromiseState::Fulfilled => {
                    let result = promise.result(scope);
                    if result.is_string() {
                        result.to_rust_string_lossy(scope)
                    } else {
                        return Err(anyhow!("Render function must return a string"));
                    }
                }
                deno_core::v8::PromiseState::Rejected => {
                    let exception = promise.result(scope);
                    let exception_str = exception.to_rust_string_lossy(scope);
                    return Err(anyhow!("Render function threw: {}", exception_str));
                }
                deno_core::v8::PromiseState::Pending => {
                    return Err(anyhow!("Render function returned unresolved promise"));
                }
            }
        } else if local.is_string() {
            local.to_rust_string_lossy(scope)
        } else {
            return Err(anyhow!("Render function must return a string"));
        }
    };

    // Extract captured console output
    let console = runtime
        .op_state()
        .borrow()
        .borrow::<ConsoleOutput>()
        .clone();

    Ok(SsrResult {
        html: html_string,
        console,
    })
}
