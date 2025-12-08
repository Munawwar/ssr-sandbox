# Examples

## Python Client

The `python_client.py` script demonstrates using ssr-sandbox in server mode from Python.

### Usage

```python
from ssr_client import SSRSandbox

with SSRSandbox("./ssr-sandbox", "./dist/chunks") as ssr:
    result = ssr.render("entry.js", {"page": "home"})
    print(result.body)  # HTML output
```

### Running the Example

```bash
# Build the sandbox and example JS bundle
cargo build --release
./build-ssr.sh

# Run the Python client
python3 examples/python_client.py
```

### Protocol

The server mode uses a simple line-based protocol over stdin/stdout:

**Request** (2 lines on stdin):
```
entry.js
{"page":"home"}
```

**Response** (stdout):
```
Status:Ok
Length:1234

<!DOCTYPE html>...
```

Error responses have `Status:Error` with the error message as the body.

### Performance

| Metric | Time |
|--------|------|
| First render (cold start) | ~10ms |
| Subsequent renders | ~0.2ms |

Server mode achieves ~30x speedup by reusing the V8 isolate and caching render functions.
