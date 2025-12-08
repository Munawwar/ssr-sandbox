//! Sandboxed module loader that only allows loading JS from a specific directory.
//! Blocks all network access, filesystem escape, and restricts to .js/.mjs files.

use deno_core::{
    anyhow::{anyhow, Error},
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier,
    ModuleType, RequestedModuleType, ResolutionKind,
};
use std::path::{Path, PathBuf};

/// A module loader that restricts all imports to a single directory.
///
/// Security guarantees:
/// - No network access (http/https URLs rejected)
/// - No filesystem escape (path traversal blocked via canonicalization)
/// - Only .js and .mjs files allowed
/// - Dynamic imports supported but sandboxed
pub struct SandboxedLoader {
    allowed_dir: PathBuf,
}

impl SandboxedLoader {
    /// Create a new sandboxed loader that only allows loading from `allowed_dir`.
    ///
    /// # Panics
    /// Panics if `allowed_dir` doesn't exist or can't be canonicalized.
    pub fn new(allowed_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let canonical = allowed_dir
            .as_ref()
            .canonicalize()
            .map_err(|e| anyhow!("Failed to canonicalize allowed_dir: {}", e))?;

        if !canonical.is_dir() {
            return Err(anyhow!("allowed_dir must be a directory"));
        }

        Ok(Self {
            allowed_dir: canonical,
        })
    }

    /// Check if a path is within the allowed directory.
    /// Uses canonicalization to resolve symlinks and prevent traversal.
    fn is_path_allowed(&self, path: &Path) -> bool {
        match path.canonicalize() {
            Ok(canonical) => canonical.starts_with(&self.allowed_dir),
            Err(_) => false,
        }
    }

    /// Validate file extension is allowed (.js or .mjs only)
    fn is_extension_allowed(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("js") | Some("mjs")
        )
    }
}

impl ModuleLoader for SandboxedLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
        // Block all remote URLs
        if specifier.starts_with("http://")
            || specifier.starts_with("https://")
            || specifier.starts_with("data:")
            || specifier.starts_with("blob:")
        {
            return Err(anyhow!(
                "Remote imports are forbidden: {}",
                specifier
            ));
        }

        // Resolve the specifier
        let resolved = if specifier.starts_with("./") || specifier.starts_with("../") {
            // Relative import - resolve against referrer
            let referrer_url = ModuleSpecifier::parse(referrer)
                .map_err(|e| anyhow!("Invalid referrer '{}': {}", referrer, e))?;
            referrer_url
                .join(specifier)
                .map_err(|e| anyhow!("Failed to resolve '{}': {}", specifier, e))?
        } else if specifier.starts_with("file://") {
            // Absolute file URL
            ModuleSpecifier::parse(specifier)
                .map_err(|e| anyhow!("Invalid file URL '{}': {}", specifier, e))?
        } else if specifier.starts_with('/') {
            // Absolute path - convert to file URL
            ModuleSpecifier::from_file_path(specifier)
                .map_err(|_| anyhow!("Invalid absolute path: {}", specifier))?
        } else {
            // Bare specifier - resolve from allowed_dir root
            // This handles imports like "chunk-abc123.js"
            ModuleSpecifier::from_file_path(self.allowed_dir.join(specifier))
                .map_err(|_| anyhow!("Invalid bare specifier: {}", specifier))?
        };

        // Ensure it's a file:// URL
        if resolved.scheme() != "file" {
            return Err(anyhow!(
                "Only file:// URLs allowed, got: {}",
                resolved.scheme()
            ));
        }

        // Get the filesystem path
        let path = resolved
            .to_file_path()
            .map_err(|_| anyhow!("Failed to convert URL to path: {}", resolved))?;

        // Security check: path must be within allowed directory
        if !self.is_path_allowed(&path) {
            return Err(anyhow!(
                "Access denied: '{}' is outside the allowed directory",
                path.display()
            ));
        }

        // Extension check
        if !Self::is_extension_allowed(&path) {
            return Err(anyhow!(
                "Only .js and .mjs files allowed, got: {}",
                path.display()
            ));
        }

        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let specifier = module_specifier.clone();

        // Convert to path
        let path = match specifier.to_file_path() {
            Ok(p) => p,
            Err(_) => {
                return ModuleLoadResponse::Sync(Err(anyhow!(
                    "Invalid file path: {}",
                    specifier
                )));
            }
        };

        // Defense in depth: re-check path is allowed
        if !self.is_path_allowed(&path) {
            return ModuleLoadResponse::Sync(Err(anyhow!(
                "Access denied: {}",
                path.display()
            )));
        }

        // Defense in depth: re-check extension
        if !Self::is_extension_allowed(&path) {
            return ModuleLoadResponse::Sync(Err(anyhow!(
                "Invalid extension: {}",
                path.display()
            )));
        }

        // Load the file content
        let code = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return ModuleLoadResponse::Sync(Err(anyhow!(
                    "Failed to read '{}': {}",
                    path.display(),
                    e
                )));
            }
        };

        ModuleLoadResponse::Sync(Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleSourceCode::String(code.into()),
            &specifier,
            None,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_blocks_remote_urls() {
        let dir = tempdir().unwrap();
        let loader = SandboxedLoader::new(dir.path()).unwrap();

        let result = loader.resolve("https://evil.com/payload.js", "file:///test.js", ResolutionKind::Import);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Remote imports are forbidden"));
    }

    #[test]
    fn test_blocks_path_traversal() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.js"), "export default 1;").unwrap();
        let loader = SandboxedLoader::new(dir.path()).unwrap();

        let entry = format!("file://{}/test.js", dir.path().display());
        let result = loader.resolve("../../../etc/passwd", &entry, ResolutionKind::Import);
        assert!(result.is_err());
    }

    #[test]
    fn test_allows_valid_imports() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("chunk.js"), "export default 1;").unwrap();
        let loader = SandboxedLoader::new(dir.path()).unwrap();

        let entry = format!("file://{}/entry.js", dir.path().display());
        let result = loader.resolve("./chunk.js", &entry, ResolutionKind::Import);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocks_non_js_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();
        let loader = SandboxedLoader::new(dir.path()).unwrap();

        let entry = format!("file://{}/entry.js", dir.path().display());
        let result = loader.resolve("./data.json", &entry, ResolutionKind::Import);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Only .js and .mjs"));
    }
}
