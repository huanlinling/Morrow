//! morrow-cli — `morrow new` / `morrow build` / `morrow package`.
//!
//! Thin tool for mod authors: scaffold a cdylib mod project, build it,
//! and zip the result into a `.morrow` package the host can load.
//! No repo checkout required — the SDK comes from crates.io.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("new") => cmd_new(args.get(1).map(String::as_str)),
        Some("build") => cmd_build(),
        Some("package") => cmd_package(),
        _ => {
            eprintln!("morrow — native Minecraft mod toolchain\n");
            eprintln!("  morrow new <name>   scaffold a mod project");
            eprintln!("  morrow build        build the mod (cargo build --release)");
            eprintln!("  morrow package      zip it into <name>.morrow for the mods/ dir");
            exit(if args.is_empty() { 0 } else { 2 });
        }
    }
}

// ─── new ────────────────────────────────────────────────────────────

fn cmd_new(name: Option<&str>) {
    let Some(name) = name else {
        eprintln!("usage: morrow new <name>");
        exit(2);
    };
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        eprintln!("error: mod name must be ASCII letters, digits, '-'");
        exit(1);
    }
    let dir = Path::new(name);
    if dir.exists() {
        eprintln!("error: '{name}' already exists");
        exit(1);
    }
    fs::create_dir_all(dir.join("src")).unwrap();

    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [lib]\ncrate-type = [\"cdylib\"]\n\n\
             [dependencies]\nmorrow = \"1\"\n"
        ),
    )
    .unwrap();

    fs::write(
        dir.join("src/lib.rs"),
        "use morrow::prelude::*;\n\n\
         #[morrow::mod_main]\n\
         fn init(_ctx: &mut Context, _api: *const RuntimeApi) -> Result<(), MorrowError> {\n    \
         morrow::info!(\"{name} loaded!\");\n    \
         Ok(())\n\
         }\n\n\
         #[morrow::event(tick)]\n\
         fn on_tick(tick: u64) {\n    \
         if tick % 200 == 0 {\n        \
         morrow::info!(\"{name} tick {}\", tick);\n    \
         }\n\
         }\n"
            .replace("{name}", name),
    )
    .unwrap();

    fs::write(
        dir.join("manifest.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\ndescription = \"A Morrow mod\"\n\n\
             [morrow]\napi_version = 1\n\n\
             [entry]\nsymbol = \"morrow_mod_init\"\n"
        ),
    )
    .unwrap();

    println!("created {name}/ — next: cd {name} && morrow build && morrow package");
}

// ─── build ──────────────────────────────────────────────────────────

fn cmd_build() {
    must_be_mod_project();
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("failed to run cargo");
    exit(status.code().unwrap_or(1));
}

// ─── package ────────────────────────────────────────────────────────

fn must_be_mod_project() -> PathBuf {
    let cwd = env::current_dir().unwrap();
    if !cwd.join("manifest.toml").exists() {
        eprintln!("error: manifest.toml not found — run inside a morrow mod project");
        exit(1);
    }
    cwd
}

/// (platform dir in the .morrow zip, library file name)
fn platform_lib(cargo_name: &str) -> (String, String) {
    let arch = match env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => {
            eprintln!("error: unsupported arch {other}");
            exit(1);
        }
    };
    match env::consts::OS {
        "linux" => (format!("linux-{arch}"), format!("lib{cargo_name}.so")),
        "macos" => (format!("macos-{arch}"), format!("lib{cargo_name}.dylib")),
        "windows" => (format!("windows-{arch}"), format!("{cargo_name}.dll")),
        other => {
            eprintln!("error: unsupported OS {other}");
            exit(1);
        }
    }
}

fn cmd_package() {
    let cwd = must_be_mod_project();
    let dir_name = cwd.file_name().unwrap().to_string_lossy().to_string();
    // Cargo replaces hyphens with underscores in library names.
    let cargo_name = dir_name.replace('-', "_");
    let (platform, lib_name) = platform_lib(&cargo_name);

    let lib_src = cwd.join("target/release").join(&lib_name);
    if !lib_src.exists() {
        eprintln!("error: {} not found — run `morrow build` first", lib_src.display());
        exit(1);
    }

    let out = cwd.join(format!("{dir_name}.morrow"));
    let file = fs::File::create(&out).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    let mut add = |arc: &str, src: &Path| {
        zip.start_file(arc, opts).unwrap();
        zip.write_all(&fs::read(src).unwrap()).unwrap();
    };
    add("manifest.toml", &cwd.join("manifest.toml"));
    let config = cwd.join("config.toml");
    if config.exists() {
        add("config.toml", &config);
    }
    add(&format!("{platform}/{lib_name}"), &lib_src);
    zip.finish().unwrap();

    println!("created {} ({platform}/{lib_name})", out.display());
    println!("drop it into the server's mods/ directory");
}
