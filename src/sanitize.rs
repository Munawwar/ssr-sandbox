//! Sanitize JSON props to prevent prototype pollution attacks.
//!
//! Removes dangerous keys like `__proto__`, `constructor`, and `prototype`
//! that could be used to pollute Object.prototype in user render functions.

use anyhow::{anyhow, Result};
use serde_json::{Map, Value};

/// Maximum recursion depth for nested objects/arrays
const MAX_DEPTH: usize = 32;

/// Keys that could be used for prototype pollution
const DANGEROUS_KEYS: &[&str] = &["__proto__", "constructor", "prototype"];

/// Recursively sanitize a JSON value, erroring if dangerous keys are found.
///
/// # Errors
/// Returns an error if:
/// - A dangerous key (`__proto__`, `constructor`, `prototype`) is found
/// - Nesting depth exceeds MAX_DEPTH (32)
pub fn sanitize_props(value: Value) -> Result<Value> {
    sanitize_recursive(value, 0)
}

fn sanitize_recursive(value: Value, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(anyhow!(
            "Props nesting too deep (max {} levels) - possible DoS attempt",
            MAX_DEPTH
        ));
    }

    match value {
        Value::Object(map) => {
            // Check for dangerous keys
            for key in map.keys() {
                if DANGEROUS_KEYS.contains(&key.as_str()) {
                    return Err(anyhow!(
                        "Prototype pollution attempt: '{}' key is forbidden in props",
                        key
                    ));
                }
            }

            // Recursively sanitize all values
            let mut sanitized = Map::new();
            for (key, val) in map {
                sanitized.insert(key, sanitize_recursive(val, depth + 1)?);
            }
            Ok(Value::Object(sanitized))
        }
        Value::Array(arr) => {
            // Recursively sanitize array elements
            let sanitized: Result<Vec<Value>> = arr
                .into_iter()
                .map(|v| sanitize_recursive(v, depth + 1))
                .collect();
            Ok(Value::Array(sanitized?))
        }
        // Primitives are safe
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_safe_props() {
        let props = json!({
            "page": "home",
            "user": {
                "name": "Alice",
                "settings": {
                    "theme": "dark"
                }
            },
            "items": [1, 2, {"nested": true}]
        });

        let result = sanitize_props(props.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), props);
    }

    #[test]
    fn test_blocks_proto() {
        let props = json!({
            "__proto__": {"polluted": true}
        });

        let result = sanitize_props(props);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("__proto__"));
    }

    #[test]
    fn test_blocks_constructor() {
        let props = json!({
            "constructor": {"prototype": {}}
        });

        let result = sanitize_props(props);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("constructor"));
    }

    #[test]
    fn test_blocks_nested_proto() {
        let props = json!({
            "safe": {
                "nested": {
                    "__proto__": {"polluted": true}
                }
            }
        });

        let result = sanitize_props(props);
        assert!(result.is_err());
    }

    #[test]
    fn test_blocks_proto_in_array() {
        let props = json!({
            "items": [
                {"safe": true},
                {"__proto__": {"polluted": true}}
            ]
        });

        let result = sanitize_props(props);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_limit() {
        // Create deeply nested object
        let mut value = json!({"leaf": true});
        for _ in 0..35 {
            value = json!({"nested": value});
        }

        let result = sanitize_props(value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too deep"));
    }
}
