//! Platform artifact selection for `.morrow` packages.

/// Identify the target platform for native artifact selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}

impl Platform {
    /// Detect the current platform at runtime.
    pub fn detect() -> Self {
        Platform {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }

    /// Return the directory name inside a .morrow package for this platform.
    ///
    /// Examples: "linux-x86_64", "windows-x86_64", "macos-aarch64".
    pub fn dir_name(&self) -> String {
        let os = match self.os.as_str() {
            "linux" => "linux",
            "windows" => "windows",
            "macos" => "macos",
            other => other,
        };

        let arch = match self.arch.as_str() {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" => "aarch64",
            other => other,
        };

        format!("{os}-{arch}")
    }
}

impl Default for Platform {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_platform_dir_exists() {
        let p = Platform::detect();
        let dir = p.dir_name();
        // On our dev machine this should be "linux-x86_64"
        assert!(dir.starts_with("linux-") || dir.starts_with("windows-") || dir.starts_with("macos-"));
    }
}
