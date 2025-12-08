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
use ssr_sandbox::{create_runtime, execute_ssr, SandboxConfig};
use std::io::{BufRead, Write};
use std::path::Path;

fn print_usage() {
    eprintln!("SSR Sandbox - Secure server-side rendering runtime");
    eprintln!();
    eprintln!("Single-shot mode:");
    eprintln!("  ssr-sandbox <chunks-dir> <entry-point> [props-json]");
    eprintln!();
    eprintln!("Server mode (persistent process):");
    eprintln!("  ssr-sandbox --server <chunks-dir>");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  ssr-sandbox ./dist/chunks ./dist/chunks/entry.js '{{\"page\":\"home\"}}'");
    eprintln!("  ssr-sandbox --server ./dist/chunks");
}

/// Run in single-shot mode (original behavior)
async fn run_single_shot(chunks_dir: &str, entry_point: &str, props_json: Option<&str>) -> Result<()> {
    let props: serde_json::Value = match props_json {
        Some(json) => serde_json::from_str(json).map_err(|e| anyhow!("Invalid props JSON: {}", e))?,
        None => serde_json::json!({}),
    };

    let config = SandboxConfig {
        chunks_dir: chunks_dir.to_string(),
        ..Default::default()
    };

    let mut runtime = create_runtime(&config)?;
    let result = execute_ssr(&mut runtime, Path::new(entry_point), props).await?;

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
async fn run_server(chunks_dir: &str) -> Result<()> {
    let config = SandboxConfig {
        chunks_dir: chunks_dir.to_string(),
        ..Default::default()
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

        // Build full entry path
        let entry_path = Path::new(chunks_dir).join(entry);

        // Execute SSR (reuses the same runtime, render functions are cached in JS)
        match execute_ssr(&mut runtime, &entry_path, props).await {
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
                write_response(&mut stdout, false, &e.to_string())?;
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
        return run_server(&args[2]).await;
    }

    // Single-shot mode
    if args.len() < 3 {
        print_usage();
        return Err(anyhow!("Missing required arguments"));
    }

    let chunks_dir = &args[1];
    let entry_point = &args[2];
    let props_json = args.get(3).map(|s| s.as_str());

    run_single_shot(chunks_dir, entry_point, props_json).await
}
