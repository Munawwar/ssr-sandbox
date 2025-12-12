# SSR Sandbox

The goal is to contain security exploits / supply chain attacks in frontend JavaScript SSR (server side rendering) code from ever getting access to server resources (filesystem, env vars, network etc). Sandbox implemented with `deno_core`. Interestingly this may also allow other programming languages to do JS SSR without using full node.js / deno.

## Performance of example SSR JS bundle

| Metric | Single-shot | Server mode (first) | Server mode (subsequent) |
|--------|-------------|---------------------|--------------------------|
| Render time | 10-12ms | 10-12ms | **<0.1ms** |
| Peak RAM | ~22 MB | ~22 MB | ~22 MB |

Binary size: 48 MB (linux x86_64)

# Usage

Download the binary and check the integration example in examples directory

### JS Entry Point Format

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

## Design Considerations

- We want to utilize JS engine JIT optimizations for performance, so trying to isolate requests from each other is a non-goal.
- ESM imports and dynamic imports are allowed within a filesystem directory. External origin imports are not allowed at the moment
- We also have to make sure the JS code doesn't consume all the memory of the machine or go into infinite loop
- `fetch()` is available but restricted to explicitly allowed origins via `--allow-origin`. Redirects are only followed within the same origin.
- Not all web APIs will be implemented. We are keeping the scope limited to what's needed for a typical SSR bundle.

## Security

The sandbox blocks the following:

- Filesystem access (`fs.readFile`, etc.)
- Network access (except allowed origins via `--allow-origin`)
- Environment variables (`process.env`)
- Child processes (`child_process`)
- Dynamic imports outside sandbox directory
- Path traversal (`../../../etc/passwd`)
- Remote imports (`https://evil.com/x.js`)
- Tampering with internal render cache

And limits resource usage by default:
- Memory: 64MB heap (configurable via `--max-heap-size`)
- Time: 30s render timeout (configurable via `--timeout`)*

## Available Web APIs

The sandbox provides these standard Web APIs for SSR compatibility:

Full Support:
| API | Notes |
|-----|-------|
| `AbortController/AbortSignal` | |
| `atob/btoa` | |
| `Blob/File/FileReader` | |
| `CompressionStream/DecompressionStream` | gzip/deflate |
| `crypto.getRandomValues` | |
| `crypto.randomUUID` | |
| `crypto.subtle.*` | Full Web Crypto API |
| `DOMException` | |
| `Event/EventTarget/CustomEvent` | |
| `Intl.*` | V8 built-in |
| `MessageChannel/MessagePort` | |
| `performance.now()` | |
| `queueMicrotask` | V8 built-in |
| `ReadableStream/WritableStream/TransformStream` | |
| `structuredClone` | |
| `TextEncoder/TextDecoder` | |
| `TextEncoderStream/TextDecoderStream` | |
| `URL/URLSearchParams` | |
| `URLPattern` | |

Partial Support:
| API | Status |
|-----|--------|
| `console.log/warn/error` | Captured in Rust, not printed |
| `fetch` | Restricted to allowed origins |
| `Headers/Request/Response` | Simplified (see below) |
| `requestAnimationFrame` | Stubbed (no-op) |
| `setTimeout/setInterval` | Stubbed (no-op) |

### Fetch API Limitations

`Headers`, `Request`, and `Response` are simplified implementations that cover common SSR use cases but are not fully spec-compliant:

- `Response.body` returns the body as a string, not a `ReadableStream`
- `Response.blob()` and `Response.formData()` are not implemented
- `Request.body` is stored as a string, not a stream
- No support for `Request.cache`, `Request.credentials`, `Request.mode`, `Request.redirect` options
- `Headers` does not validate header names/values per spec

These work fine for typical SSR patterns (calling JSON APIs, fetching text content), but may not work for advanced streaming or binary use cases.

## Binary Usage

### CLI Options

| Option | Description |
|--------|-------------|
| `--max-heap-size <MB>` | Maximum V8 heap size in megabytes (default: 64). Use 0 for unlimited (not recommended). |
| `--timeout <ms>` | Maximum render time in milliseconds (default: 30000). Use 0 for unlimited (not recommended). |
| `--allow-origin <url>` | Allow `fetch()` to this origin (can be specified multiple times). Example: `--allow-origin https://api.example.com` |

**\* Timeout note:** When a render times out, the V8 isolate is terminated and recreated. This means the next request after a timeout will incur a cold start penalty (~10ms instead of ~0.2ms).

### "Server" Mode (via child process stdin/stdout)

For production use - keeps V8 warm for fast subsequent renders:

```bash
./target/release/ssr-sandbox --server [options] <chunks-dir>

# With custom heap limit
./target/release/ssr-sandbox --max-heap-size 256 --server ./dist/chunks

# With allowed fetch origin
./target/release/ssr-sandbox --allow-origin https://api.example.com --server ./dist/chunks
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

### Single-Shot Mode (mostly for testing purpose)

This is for testing purpose mainly and not really meant for production use. The example takes 10-12ms on my machine and that's not fast enough for production use.

```bash
./target/release/ssr-sandbox [options] <chunks-dir> <entry-point> [props-json]

# Example
./target/release/ssr-sandbox ./dist ./dist/entry.js '{"page":"home","user":"Alice"}'

# With custom heap limit
./target/release/ssr-sandbox --max-heap-size 256 ./dist ./dist/entry.js '{"page":"home"}'
```

### Client Examples

See the [examples/](examples/) directory for client implementations:

- **Python**: `examples/python_client.py` - Full client with timing benchmarks

## Development

### Requirements

Rust version: See `rust-version` in Cargo.toml

```bash
# Build
cargo build --release

# Single-shot mode
./target/release/ssr-sandbox ./dist/chunks ./dist/chunks/entry.js '{"page":"home"}'

# Server mode (persistent process)
./target/release/ssr-sandbox --server ./dist/chunks
```

## Cross-Compilation

```bash
# Install targets
rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu

# Build for ARM64 (e.g., AWS Graviton)
cargo build --release --target aarch64-unknown-linux-gnu

# Build for x86_64
cargo build --release --target x86_64-unknown-linux-gnu
```

## Docker (for local testing)

The Dockerfile is provided for local testing in a Linux environment. For production, download prebuilt binaries from GitHub Releases.

```bash
docker build -t ssr-sandbox:latest .
docker run --rm -v ./dist:/app/chunks:ro ssr-sandbox /app/chunks /app/chunks/entry.js '{}'
```

### Security updates

```bash
cargo update        # Update dependencies within version constraints
cargo build --release
cargo test          # Verify nothing broke
```

For major dependency upgrades, manually update version numbers in `Cargo.toml` and test thoroughly.

