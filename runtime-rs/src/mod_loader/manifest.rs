//! Manifest parsing for `.morrow` packages.
//!
//! See docs/05-package-format.md for the specification.

use serde::Deserialize;

/// Parsed manifest.toml from a .morrow package.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub package: PackageMeta,
    #[allow(dead_code)]
    pub morrow: MorrowMeta,
    pub entry: EntryMeta,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MorrowMeta {
    #[allow(dead_code)]
    pub api_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryMeta {
    /// Name of the exported entry symbol (e.g. "morrow_mod_init").
    pub symbol: String,
}

/// Parse a manifest.toml string.
pub fn parse(contents: &str) -> Result<Manifest, String> {
    toml::from_str::<Manifest>(contents).map_err(|e| format!("invalid manifest.toml: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_manifest() {
        let toml = r#"
[package]
name = "hello-morrow"
version = "0.1.0"

[morrow]
api_version = 1

[entry]
symbol = "morrow_mod_init"
"#;
        let m = parse(toml).unwrap();
        assert_eq!(m.package.name, "hello-morrow");
        assert_eq!(m.package.version, "0.1.0");
        assert_eq!(m.morrow.api_version, 1);
        assert_eq!(m.entry.symbol, "morrow_mod_init");
    }
}
