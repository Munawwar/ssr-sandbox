//! Build script to create V8 snapshot for faster cold starts.
//!
//! This pre-compiles and evaluates all extension JS modules at build time,
//! so runtime only needs to deserialize the snapshot instead of parsing/compiling JS.

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

// Include the shared ops module using #[path] attribute
// This ensures ops are IDENTICAL between build.rs and runtime
#[path = "src/ops.rs"]
mod ops;

fn main() {
    // Tell Cargo to rerun if these files change
    println!("cargo:rerun-if-changed=src/bootstrap.js");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ops.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("SSR_SNAPSHOT.bin");

    // Create blob store for deno_web (required even during snapshot creation)
    let blob_store = Arc::new(deno_web::BlobStore::default());

    let snapshot = deno_core::snapshot::create_snapshot(
        deno_core::snapshot::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            startup_snapshot: None,
            skip_op_registration: false,
            extensions: vec![
                // Deno extensions (order matters - dependencies first)
                deno_webidl::deno_webidl::init_ops_and_esm(),
                deno_console::deno_console::init_ops_and_esm(),
                deno_url::deno_url::init_ops_and_esm(),
                deno_web::deno_web::init_ops_and_esm::<deno_permissions::PermissionsContainer>(
                    blob_store,
                    None,
                ),
                deno_crypto::deno_crypto::init_ops_and_esm(None),
                // Our custom extension (from shared ops module)
                ops::ssr_runtime::init_ops_and_esm(),
            ],
            with_runtime_cb: None,
            extension_transpiler: None,
        },
        None, // No warmup script
    )
    .expect("Failed to create snapshot");

    std::fs::write(&snapshot_path, snapshot.output).expect("Failed to write snapshot");

    println!(
        "cargo:warning=Snapshot created at {:?} ({} bytes)",
        snapshot_path,
        std::fs::metadata(&snapshot_path).unwrap().len()
    );
}
