//! Mod loader — reads `.morrow` packages, selects platform artifacts,
//! loads native libraries, and calls mod entry points.
//!
//! See docs/05-package-format.md and docs/03-lifecycle.md.

pub mod artifact;
pub mod manifest;

use artifact::Platform;
use manifest::Manifest;
use crate::host_api::RuntimeApi;
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
    /// Optional tick callback, discovered from `morrow_mod_tick` export.
    #[allow(dead_code)]
    pub tick_callback: Option<unsafe extern "C" fn(u64)>,
    /// Optional lifecycle: server started.
    #[allow(dead_code)]
    pub server_start_callback: Option<unsafe extern "C" fn()>,
    /// Optional lifecycle: server stopping.
    #[allow(dead_code)]
    pub server_stop_callback: Option<unsafe extern "C" fn()>,
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

    pub fn has(&self, name: &str) -> bool {
        self.mods.contains_key(name)
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

/// Load a `.morrow` package from a file path.
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
/// Read config.toml from a .morrow package without loading the mod.
pub fn read_zip_config(package_path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(package_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    read_zip_entry_optional(&mut archive, "config.toml")
}

/// Discovered optional exports from a mod.
pub struct ModExports {
    pub tick_callback: Option<unsafe extern "C" fn(u64)>,
    pub server_start_callback: Option<unsafe extern "C" fn()>,
    pub server_stop_callback: Option<unsafe extern "C" fn()>,
    pub player_join_callback: Option<unsafe extern "C" fn(*const u8, u32)>,
    pub player_leave_callback: Option<unsafe extern "C" fn(*const u8, u32)>,
    pub chat_message_callback: Option<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>,
    pub block_break_callback: Option<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>,
    pub block_place_callback: Option<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>,
    pub player_death_callback: Option<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>,
}

/// Returns the mod name and discovered optional exports on success.
pub fn load_package(
    package_path: &Path,
    registry: &mut ModRegistry,
) -> Result<(String, ModExports), String> {
    // 1. Open ZIP
    let file = std::fs::File::open(package_path)
        .map_err(|e| format!("cannot open {}: {e}", package_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("invalid .morrow package: {e}"))?;

    // 2. Read manifest.toml
    let manifest_contents = read_zip_entry(&mut archive, "manifest.toml")?;
    let manifest = manifest::parse(&manifest_contents)?;

    // 3. Check dependencies
    for (dep_name, dep_version) in &manifest.dependencies {
        if !registry.has(dep_name) {
            return Err(format!(
                "dependency '{dep_name} {dep_version}' not found — load it first"
            ));
        }
    }

    // 3b. Read optional config.toml
    let config_data = read_zip_entry_optional(&mut archive, "config.toml");

    eprintln!(
        "[Morrow] Loading mod: {} v{}",
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

    // 7-8. Build RuntimeApi and call entry point
    let api = RuntimeApi::new();
    let entry_symbol = &manifest.entry.symbol;
    unsafe {
        let entry: libloading::Symbol<unsafe extern "C" fn(*const RuntimeApi) -> u32> = library
            .get(entry_symbol.as_bytes())
            .map_err(|e| format!("entry symbol '{entry_symbol}' not found: {e}"))?;

        let status = entry(&api as *const RuntimeApi);
        if status != 0 {
            return Err(format!(
                "mod '{}' entry point returned error code {status}",
                manifest.package.name
            ));
        }
    }

    // 9. Discover optional exports
    let tick_callback: Option<unsafe extern "C" fn(u64)> = unsafe {
        library.get::<unsafe extern "C" fn(u64)>(b"morrow_mod_tick")
            .ok().map(|sym| *sym.into_raw())
    };
    let server_start_callback: Option<unsafe extern "C" fn()> = unsafe {
        library.get::<unsafe extern "C" fn()>(b"morrow_mod_server_start")
            .ok().map(|sym| *sym.into_raw())
    };
    let server_stop_callback: Option<unsafe extern "C" fn()> = unsafe {
        library.get::<unsafe extern "C" fn()>(b"morrow_mod_server_stop")
            .ok().map(|sym| *sym.into_raw())
    };
    let player_join_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32)>(b"morrow_mod_player_join")
            .ok().map(|sym| *sym.into_raw())
    };
    let player_leave_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32)>(b"morrow_mod_player_leave")
            .ok().map(|sym| *sym.into_raw())
    };
    let chat_message_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>(b"morrow_mod_chat_message")
            .ok().map(|sym| *sym.into_raw())
    };
    let block_break_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>(b"morrow_mod_block_break")
            .ok().map(|sym| *sym.into_raw())
    };
    let block_place_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>(b"morrow_mod_block_place")
            .ok().map(|sym| *sym.into_raw())
    };
    let player_death_callback = unsafe {
        library.get::<unsafe extern "C" fn(*const u8, u32, *const u8, u32)>(b"morrow_mod_player_death")
            .ok().map(|sym| *sym.into_raw())
    };

    if tick_callback.is_some() {
        eprintln!("[Morrow]   Optional: morrow_mod_tick");
    }
    if server_start_callback.is_some() {
        eprintln!("[Morrow]   Optional: morrow_mod_server_start");
    }
    if server_stop_callback.is_some() {
        eprintln!("[Morrow]   Optional: morrow_mod_server_stop");
    }

    // 10. Track
    let exports = ModExports {
        tick_callback,
        server_start_callback,
        server_stop_callback,
        player_join_callback,
        player_leave_callback,
        chat_message_callback,
        block_break_callback,
        block_place_callback,
        player_death_callback,
    };
    let name = manifest.package.name.clone();
    registry.insert(
        name.clone(),
        LoadedMod {
            manifest,
            library,
            temp_dir,
            tick_callback: exports.tick_callback,
            server_start_callback: exports.server_start_callback,
            server_stop_callback: exports.server_stop_callback,
        },
    );

    eprintln!("[Morrow] Loaded mod: {name}");
    Ok((name, exports))
}

// ─── Helpers ──────────────────────────────────

fn read_zip_entry_optional(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

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
        "[Morrow]   Extracted {} ({:.1} KB)",
        lib_filename,
        buf.len() as f64 / 1024.0
    );

    Ok((Some(tmp), dest))
}

fn is_native_lib(name: &str) -> bool {
    name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")
}
