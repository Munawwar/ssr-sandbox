//! SSR Sandbox CLI
//!
//! Single-shot mode:
//!   ssr-sandbox <chunks-dir> <entry-point> [props-json]
//!
//! Server mode (persistent process, reads from stdin):
//!   ssr-sandbox --server <chunks-dir>
//!
//! Protocol (server mode):
//!   Request (stdin):
//!     entry.js
//!     {"page":"home","user":"Alice"}
//!
//!   Response (stdout):
//!     Status:Ok
//!     Length:1234
//!
//!     <!DOCTYPE html>...
//!
//!   Error response:
//!     Status:Error
//!     Length:42
//!
//!     Render function threw: undefined is not...

use anyhow::{anyhow, Result};
use ssr_sandbox::{create_runtime, execute_ssr, sanitize_props, SandboxConfig};
use std::io::{BufRead, Write};
use std::path::Path;

fn print_usage() {
    eprintln!("SSR Sandbox - Secure server-side rendering runtime");
    eprintln!();
    eprintln!("Single-shot mode:");
    eprintln!("  ssr-sandbox [options] <chunks-dir> <entry-point> [props-json]");
    eprintln!();
    eprintln!("Server mode (persistent process):");
    eprintln!("  ssr-sandbox --server [options] <chunks-dir>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --max-heap-size <MB>  Maximum V8 heap size in megabytes (default: 64)");
    eprintln!("                        Use 0 for unlimited (not recommended)");
    eprintln!("  --timeout <ms>        Maximum render time in milliseconds (default: 5000)");
    eprintln!("                        Use 0 for unlimited (not recommended)");
    eprintln!("  --allow-origin <url>  Allow fetch() to this origin (can be specified multiple times)");
    eprintln!("                        Example: --allow-origin https://api.example.com");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  ssr-sandbox ./dist/chunks ./dist/chunks/entry.js '{{\"page\":\"home\"}}'");
    eprintln!("  ssr-sandbox --server ./dist/chunks");
    eprintln!("  ssr-sandbox --timeout 5000 --server ./dist/chunks");
    eprintln!("  ssr-sandbox --allow-origin https://api.example.com --server ./dist/chunks");
}

fn parse_heap_size(args: &[String]) -> Option<usize> {
    for i in 0..args.len() {
        if args[i] == "--max-heap-size" {
            if let Some(size_str) = args.get(i + 1) {
                if let Ok(mb) = size_str.parse::<usize>() {
                    return Some(if mb == 0 { 0 } else { mb * 1024 * 1024 });
                }
            }
        }
    }
    None
}

fn parse_timeout(args: &[String]) -> Option<u64> {
    for i in 0..args.len() {
        if args[i] == "--timeout" {
            if let Some(ms_str) = args.get(i + 1) {
                if let Ok(ms) = ms_str.parse::<u64>() {
                    return Some(ms);
                }
            }
        }
    }
    None
}

fn parse_allowed_origins(args: &[String]) -> Vec<String> {
    let mut origins = vec![];
    for i in 0..args.len() {
        if args[i] == "--allow-origin" {
            if let Some(origin) = args.get(i + 1) {
                origins.push(origin.clone());
            }
        }
    }
    origins
}

fn filter_options(args: &[String]) -> Vec<String> {
    let mut result = vec![args[0].clone()];
    let mut skip_next = false;
    for arg in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--max-heap-size" || arg == "--timeout" || arg == "--allow-origin" {
            skip_next = true;
            continue;
        }
        result.push(arg.clone());
    }
    result
}

/// Run in single-shot mode (original behavior)
async fn run_single_shot(chunks_dir: &str, entry_point: &str, props_json: Option<&str>, max_heap_size: Option<usize>, timeout_ms: Option<u64>, allowed_origins: Vec<String>) -> Result<()> {
    let props: serde_json::Value = match props_json {
        Some(json) => serde_json::from_str(json).map_err(|e| anyhow!("Invalid props JSON: {}", e))?,
        None => serde_json::json!({}),
    };

    // Sanitize props to prevent prototype pollution
    let props = sanitize_props(props)?;

    let config = SandboxConfig {
        chunks_dir: chunks_dir.to_string(),
        max_heap_size: max_heap_size.or(Some(64 * 1024 * 1024)),
        timeout_ms: timeout_ms.or(Some(5_000)),
        allowed_origins,
    };

    let mut runtime = create_runtime(&config)?;
    let result = execute_ssr(&mut runtime, Path::new(entry_point), props, config.timeout_ms).await?;

    // Print captured console output to stderr
    for log in &result.console.logs {
        eprintln!("[LOG] {}", log);
    }
    for warn in &result.console.warns {
        eprintln!("[WARN] {}", warn);
    }
    for err in &result.console.errors {
        eprintln!("[ERROR] {}", err);
    }

    // Print HTML to stdout
    println!("{}", result.html);

    Ok(())
}

/// Run in server mode (persistent process, reads requests from stdin)
async fn run_server(chunks_dir: &str, max_heap_size: Option<usize>, timeout_ms: Option<u64>, allowed_origins: Vec<String>) -> Result<()> {
    let config = SandboxConfig {
        chunks_dir: chunks_dir.to_string(),
        max_heap_size: max_heap_size.or(Some(64 * 1024 * 1024)),
        timeout_ms: timeout_ms.or(Some(5_000)),
        allowed_origins,
    };

    // Create runtime ONCE at startup (V8 cold start happens here)
    let mut runtime = create_runtime(&config)?;

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader = stdin.lock();

    // Signal ready
    eprintln!("[ssr-sandbox] Server ready, reading from stdin...");

    loop {
        let mut entry_line = String::new();
        let mut props_line = String::new();

        // Read entry point (line 1)
        let bytes_read = reader.read_line(&mut entry_line)?;
        if bytes_read == 0 {
            // EOF - stdin closed, exit gracefully
            break;
        }

        // Read props JSON (line 2)
        reader.read_line(&mut props_line)?;

        let entry = entry_line.trim();
        let props_str = props_line.trim();

        // Parse props
        let props: serde_json::Value = if props_str.is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str(props_str) {
                Ok(p) => p,
                Err(e) => {
                    let error_msg = format!("Invalid props JSON: {}", e);
                    write_response(&mut stdout, false, &error_msg)?;
                    continue;
                }
            }
        };

        // Sanitize props to prevent prototype pollution
        let props = match sanitize_props(props) {
            Ok(p) => p,
            Err(e) => {
                write_response(&mut stdout, false, &e.to_string())?;
                continue;
            }
        };

        // Build full entry path
        let entry_path = Path::new(chunks_dir).join(entry);

        // Execute SSR (reuses the same runtime, render functions are cached in JS)
        match execute_ssr(&mut runtime, &entry_path, props, config.timeout_ms).await {
            Ok(result) => {
                // Log console output to stderr
                for log in &result.console.logs {
                    eprintln!("[LOG] {}", log);
                }
                for warn in &result.console.warns {
                    eprintln!("[WARN] {}", warn);
                }
                for err in &result.console.errors {
                    eprintln!("[ERROR] {}", err);
                }

                write_response(&mut stdout, true, &result.html)?;
            }
            Err(e) => {
                let err_msg = e.to_string();
                let is_timeout = err_msg.contains("timed out");
                write_response(&mut stdout, false, &err_msg)?;

                // After a timeout, the V8 isolate may be in a bad state
                // Recreate it to ensure subsequent requests work correctly
                if is_timeout {
                    eprintln!("[ssr-sandbox] Recreating runtime after timeout");
                    runtime = create_runtime(&config)?;
                }
            }
        }

        // Clear console output for next request
        runtime.op_state().borrow_mut().put(ssr_sandbox::ConsoleOutput::default());
    }

    eprintln!("[ssr-sandbox] Server shutting down");
    Ok(())
}

/// Write response in length-prefixed protocol
fn write_response(stdout: &mut std::io::Stdout, ok: bool, body: &str) -> Result<()> {
    let status = if ok { "Ok" } else { "Error" };
    let length = body.len();

    writeln!(stdout, "Status:{}", status)?;
    writeln!(stdout, "Length:{}", length)?;
    writeln!(stdout)?; // Empty line separator
    write!(stdout, "{}", body)?;
    stdout.flush()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Parse options before filtering
    let max_heap_size = parse_heap_size(&args);
    // Convert 0 to None (unlimited)
    let max_heap_size = max_heap_size.and_then(|s| if s == 0 { None } else { Some(s) });

    let timeout_ms = parse_timeout(&args);
    // Convert 0 to None (unlimited)
    let timeout_ms = timeout_ms.and_then(|t| if t == 0 { None } else { Some(t) });

    let allowed_origins = parse_allowed_origins(&args);

    // Filter out options to get positional args
    let args = filter_options(&args);

    if args.len() < 2 {
        print_usage();
        return Err(anyhow!("Missing required arguments"));
    }

    // Check for server mode
    if args[1] == "--server" {
        if args.len() < 3 {
            print_usage();
            return Err(anyhow!("Server mode requires chunks-dir argument"));
        }
        return run_server(&args[2], max_heap_size, timeout_ms, allowed_origins).await;
    }

    // Single-shot mode
    if args.len() < 3 {
        print_usage();
        return Err(anyhow!("Missing required arguments"));
    }

    let chunks_dir = &args[1];
    let entry_point = &args[2];
    let props_json = args.get(3).map(|s| s.as_str());

    run_single_shot(chunks_dir, entry_point, props_json, max_heap_size, timeout_ms, allowed_origins).await
}
