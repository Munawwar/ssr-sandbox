//! # SSR Sandbox
//!
//! A minimal, secure runtime for server-side rendering using deno_core.
//!
//! ## Security Guarantees
//!
//! - **No filesystem access**: Only the specified chunks directory is readable
//! - **No network access**: All HTTP/HTTPS imports are blocked
//! - **No environment access**: `process.env`, `Deno.env` don't exist
//! - **No shell access**: No `child_process`, `Deno.run`, etc.
//! - **Dynamic imports sandboxed**: `import()` only works within chunks dir
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ssr_sandbox::{create_runtime, execute_ssr, SandboxConfig};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = SandboxConfig {
//!         chunks_dir: "./dist/chunks".into(),
//!         ..Default::default()
//!     };
//!
//!     let mut runtime = create_runtime(&config).unwrap();
//!     let result = execute_ssr(
//!         &mut runtime,
//!         Path::new("./dist/chunks/entry-server.js"),
//!         serde_json::json!({ "url": "/page" }),
//!     ).await.unwrap();
//!
//!     println!("{}", result.html);
//! }
//! ```

mod fetch;
mod loader;
mod runtime;

pub use fetch::FetchConfig;
pub use loader::SandboxedLoader;
pub use runtime::{create_runtime, execute_ssr, ConsoleOutput, SandboxConfig, SsrResult};
