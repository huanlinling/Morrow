//! Mod loader — reads `.ferrum` packages, selects platform artifacts,
//! loads native libraries, and calls mod entry points.
//!
//! See docs/05-package-format.md and docs/03-lifecycle.md.

pub mod artifact;
pub mod manifest;

use artifact::Platform;
use manifest::Manifest;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Loaded mod tracking
// ---------------------------------------------------------------------------

/// Information about a loaded mod.
pub struct LoadedMod {
    /// The parsed manifest.
    #[allow(dead_code)]
    pub manifest: Manifest,
    /// Handle to the dynamically loaded library.
    #[allow(dead_code)] // kept alive so the library stays loaded
    library: libloading::Library,
    /// Temp directory where the artifact was extracted (cleaned up on drop).
    #[allow(dead_code)]
    temp_dir: Option<tempfile::TempDir>,
    /// Optional tick callback, discovered from `ferrum_mod_tick` export.
    #[allow(dead_code)]
    pub tick_callback: Option<unsafe extern "C" fn(u64)>,
}

/// Registry of all loaded mods.
pub struct ModRegistry {
    mods: HashMap<String, LoadedMod>,
}

impl ModRegistry {
    pub fn new() -> Self {
        ModRegistry {
            mods: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, loaded: LoadedMod) {
        self.mods.insert(name, loaded);
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&LoadedMod> {
        self.mods.get(name)
    }

    pub fn len(&self) -> usize {
        self.mods.len()
    }

    /// Unload all mods (drop their libraries).
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.mods.clear();
    }
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Load a `.ferrum` package from a file path.
///
/// Steps:
/// 1. Open the ZIP file
/// 2. Read and parse `manifest.toml`
/// 3. Determine the current platform
/// 4. Extract the platform-specific artifact from the ZIP
/// 5. Write the artifact to a temp file
/// 6. `dlopen` the temp file
/// 7. Look up the entry symbol
/// 8. Call the entry point
/// 9. Track the loaded mod in the registry
///
/// Returns the mod name and optional tick callback on success,
/// or an error message on failure.
pub fn load_package(
    package_path: &Path,
    registry: &mut ModRegistry,
) -> Result<(String, Option<unsafe extern "C" fn(u64)>), String> {
    // 1. Open ZIP
    let file = std::fs::File::open(package_path)
        .map_err(|e| format!("cannot open {}: {e}", package_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("invalid .ferrum package: {e}"))?;

    // 2. Read manifest.toml
    let manifest_contents = read_zip_entry(&mut archive, "manifest.toml")?;
    let manifest = manifest::parse(&manifest_contents)?;

    eprintln!(
        "[Ferrum] Loading mod: {} v{}",
        manifest.package.name, manifest.package.version
    );

    // 3. Determine platform
    let platform = Platform::detect();
    let artifact_path = format!("{}/", platform.dir_name());

    // 4-5. Find and extract the artifact for this platform
    let (temp_dir, lib_path) = extract_artifact(&mut archive, &artifact_path, &manifest)?;

    // 6. dlopen
    let library = unsafe {
        libloading::Library::new(&lib_path)
            .map_err(|e| format!("failed to load native mod library: {e}"))?
    };

    // 7-8. Look up and call entry point
    let entry_symbol = &manifest.entry.symbol;
    unsafe {
        let entry: libloading::Symbol<unsafe extern "C" fn() -> u32> = library
            .get(entry_symbol.as_bytes())
            .map_err(|e| format!("entry symbol '{entry_symbol}' not found: {e}"))?;

        let status = entry();
        if status != 0 {
            return Err(format!(
                "mod '{}' entry point returned error code {status}",
                manifest.package.name
            ));
        }
    }

    // 9. Discover optional exports: ferrum_mod_tick
    let tick_callback: Option<unsafe extern "C" fn(u64)> = unsafe {
        library
            .get::<unsafe extern "C" fn(u64)>(b"ferrum_mod_tick")
            .ok()
            .map(|sym| *sym.into_raw())
    };

    if tick_callback.is_some() {
        eprintln!(
            "[Ferrum]   Discovered optional export: ferrum_mod_tick"
        );
    }

    // 10. Track
    let name = manifest.package.name.clone();
    registry.insert(
        name.clone(),
        LoadedMod {
            manifest,
            library,
            temp_dir,
            tick_callback,
        },
    );

    eprintln!("[Ferrum] Loaded mod: {name}");
    Ok((name, tick_callback))
}

// ─── Helpers ──────────────────────────────────

fn read_zip_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<String, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| format!("'{name}' not found in package"))?;

    let mut contents = String::new();
    entry
        .read_to_string(&mut contents)
        .map_err(|e| format!("failed to read '{name}': {e}"))?;

    Ok(contents)
}

/// Find the first .so/.dll/.dylib in the platform directory,
/// extract it to a temp file, and return the temp dir + path.
fn extract_artifact(
    archive: &mut zip::ZipArchive<std::fs::File>,
    platform_dir: &str,
    _manifest: &Manifest,
) -> Result<(Option<tempfile::TempDir>, PathBuf), String> {
    // Scan for native libs in the platform directory
    let mut lib_entry_name: Option<String> = None;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("zip error: {e}"))?;
        let name = entry.name().to_string();

        if name.starts_with(platform_dir) && is_native_lib(&name) {
            lib_entry_name = Some(name);
            break;
        }
    }

    let lib_name = lib_entry_name
        .ok_or_else(|| format!("no native artifact found for platform '{}'", platform_dir.trim_end_matches('/')))?;

    // Extract to temp dir
    let tmp = tempfile::tempdir().map_err(|e| format!("temp dir: {e}"))?;
    let lib_filename = Path::new(&lib_name)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let dest = tmp.path().join(&lib_filename);

    let mut entry = archive
        .by_name(&lib_name)
        .map_err(|_| format!("artifact '{lib_name}' not found"))?;

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("read artifact: {e}"))?;

    std::fs::write(&dest, &buf).map_err(|e| format!("write artifact: {e}"))?;

    eprintln!(
        "[Ferrum]   Extracted {} ({:.1} KB)",
        lib_filename,
        buf.len() as f64 / 1024.0
    );

    Ok((Some(tmp), dest))
}

fn is_native_lib(name: &str) -> bool {
    name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")
}
