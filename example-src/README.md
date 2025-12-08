# Example SSR Source

This directory contains example SSR source files that demonstrate the sandbox capabilities.

## Building

Use the included build script with esbuild:

```bash
./build-ssr.sh
```

Or manually with esbuild:

```bash
esbuild example-src/entry.js \
  --bundle \
  --format=esm \
  --splitting \
  --outdir=example-dist \
  --platform=neutral \
  --target=es2023
```

## Structure

- `entry.js` - Main entry point that exports the render function
- `components/` - Shared components (header, footer)
- `pages/` - Page components loaded dynamically based on props

## Testing

After building, run with:

```bash
./target/release/ssr-sandbox example-dist example-dist/entry.js '{"page":"home"}'
```

Available pages: `home`, `about`, `contact`, `apis`, `tamper`
