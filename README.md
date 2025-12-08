# SSR Sandbox

A secure, high-performance server-side rendering runtime using `deno_core`. Designed to contain npm supply chain attacks by executing untrusted JavaScript in a sandboxed V8 isolate with **zero ambient capabilities**.

## Features

- **Secure by default** - No filesystem, network, or environment access
- **Fast** - Sub-millisecond renders after warmup (~0.1-0.2ms)
- **Code splitting support** - Works with esbuild chunked bundles
- **Dynamic imports** - Sandboxed to allowed directory only
- **Server mode** - Persistent process for high-throughput SSR
- **Memory limits** - Configurable V8 heap size to prevent OOM crashes

## Quick Start

```bash
# Build
cargo build --release

# Single-shot mode
./target/release/ssr-sandbox ./dist/chunks ./dist/chunks/entry.js '{"page":"home"}'

# Server mode (persistent process)
./target/release/ssr-sandbox --server ./dist/chunks
```

## Security Guarantees

| Attack Vector | Status |
|--------------|--------|
| Filesystem access (`fs.readFile`, etc.) | Blocked |
| Network access (`fetch`, `http`) | Blocked |
| Environment variables (`process.env`) | Blocked |
| Child processes (`child_process`) | Blocked |
| Dynamic imports outside sandbox | Blocked |
| Path traversal (`../../../etc/passwd`) | Blocked |
| Remote imports (`https://evil.com/x.js`) | Blocked |
| Tampering with internal render cache | Blocked |
| Memory exhaustion (OOM) | Limited (64MB default) |
| Infinite loops / slow code | Timeout (30s default)* |

## Available Web APIs

The sandbox provides these standard Web APIs for SSR compatibility:

| API | Status |
|-----|--------|
| `console.log/warn/error` | ✓ Captured to Rust |
| `TextEncoder/TextDecoder` | ✓ |
| `URL/URLSearchParams` | ✓ |
| `atob/btoa` | ✓ |
| `crypto.randomUUID` | ✓ |
| `crypto.getRandomValues` | ✓ |
| `crypto.subtle.digest` | ✓ SHA-256/384/512 |
| `Intl.*` | ✓ V8 built-in |
| `setTimeout/setInterval` | Stubbed (no-op) |
| `requestAnimationFrame` | Stubbed (no-op) |
| `queueMicrotask` | ✓ V8 built-in |

## Usage

### Options

| Option | Description |
|--------|-------------|
| `--max-heap-size <MB>` | Maximum V8 heap size in megabytes (default: 64). Use 0 for unlimited (not recommended). |
| `--timeout <ms>` | Maximum render time in milliseconds (default: 30000). Use 0 for unlimited (not recommended). |

**\* Timeout note:** When a render times out, the V8 isolate is terminated and recreated. This means the next request after a timeout will incur a cold start penalty (~10ms instead of ~0.2ms). Timeouts should be rare in production.

### Single-Shot Mode

For one-off renders or testing:

```bash
./target/release/ssr-sandbox [options] <chunks-dir> <entry-point> [props-json]

# Example
./target/release/ssr-sandbox ./dist ./dist/entry.js '{"page":"home","user":"Alice"}'

# With custom heap limit
./target/release/ssr-sandbox --max-heap-size 256 ./dist ./dist/entry.js '{"page":"home"}'
```

### "Server" Mode (via child process stdin/stdout)

For production use - keeps V8 warm for fast subsequent renders:

```bash
./target/release/ssr-sandbox --server [options] <chunks-dir>

# With custom heap limit
./target/release/ssr-sandbox --max-heap-size 256 --server ./dist/chunks
```

Protocol (stdin/stdout):
```
# Request (2 lines)
entry.js
{"page":"home"}

# Response
Status:Ok
Length:1234

<!DOCTYPE html>...
```

### Client Examples

See the [examples/](examples/) directory for client implementations:

- **Python**: `examples/python_client.py` - Full client with timing benchmarks

## Building SSR Bundles

Use the included build script with esbuild:

```bash
./build-ssr.sh
```

Or manually with esbuild:

```bash
esbuild src/entry-server.js \
  --bundle \
  --format=esm \
  --splitting \
  --outdir=dist/chunks \
  --platform=neutral \
  --target=es2023
```

### Entry Point Format

Your entry module should export a render function:

```javascript
// entry.js
export default async function render(props) {
  // Dynamic imports work (within sandbox)
  const { Header } = await import('./components/header.js');

  return `<!DOCTYPE html>
    <html>
      <body>${Header(props)}</body>
    </html>`;
}
```

## Performance

| Metric | Single-shot | Server mode (first) | Server mode (subsequent) |
|--------|-------------|---------------------|--------------------------|
| Render time | ~11ms | ~8ms | **~0.2ms** |
| Peak RAM | ~22 MB | ~22 MB | ~22 MB |

Server mode achieves **~30x speedup** by reusing the V8 isolate and caching render functions.

## Cross-Compilation

```bash
# Install targets
rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu

# Build for ARM64 (e.g., AWS Graviton)
cargo build --release --target aarch64-unknown-linux-gnu

# Build for x86_64
cargo build --release --target x86_64-unknown-linux-gnu
```

## Docker

### Distroless (minimal)

```bash
docker build -t ssr-sandbox:latest .
```

### Amazon Linux (for Lambda/EC2)

```bash
docker build -f Dockerfile.amazonlinux -t ssr-sandbox:al2023 .
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Your Server                     │
├─────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────┐  │
│  │           SSR Sandbox (Rust)              │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │         V8 Isolate                  │  │  │
│  │  │  ┌───────────────────────────────┐  │  │  │
│  │  │  │     Your JS Bundle            │  │  │  │
│  │  │  │  (potentially compromised)    │  │  │  │
│  │  │  └───────────────────────────────┘  │  │  │
│  │  │                                     │  │  │
│  │  │  No fs │ No net │ No env │ No exec  │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  Full server capabilities (outside sandbox)     │
└─────────────────────────────────────────────────┘
```

## License

MIT
