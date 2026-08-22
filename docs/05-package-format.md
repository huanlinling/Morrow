# 05 — .mor 包格式规范

## 概述

`.mor` 是 Morrow Mod 的分发格式。它是一个 ZIP 文件（使用 store 压缩，优先速度），包含 metadata + 多平台 native artifacts + 可选的 assets。

## 版本

当前版本：**1.0**

## 文件结构

```
my-mod.mor
│
├── manifest.toml                 # 必需：包元数据
│
├── windows-x86_64/
│   └── my_mod.dll                 # Windows x86_64 native artifact
│
├── linux-x86_64/
│   └── libmy_mod.so              # Linux x86_64 native artifact
│
├── linux-aarch64/                 # 可选：ARM64 Linux
│   └── libmy_mod.so
│
├── macos-x86_64/                  # v2 预留
│   └── libmy_mod.dylib
│
├── macos-aarch64/                 # v2 预留 (Apple Silicon)
│   └── libmy_mod.dylib
│
└── assets/                        # 可选：资源文件
    ├── textures/
    │   └── ...
    └── sounds/
        └── ...
```

## 平台三元组

| 平台 ID | os.name | os.arch | Native 扩展名 |
|---------|---------|---------|---------------|
| `windows-x86_64` | Windows | amd64 | `.dll` |
| `linux-x86_64` | Linux | amd64 | `.so` |
| `linux-aarch64` | Linux | aarch64 | `.so` |
| `macos-x86_64` | Mac OS X | x86_64 | `.dylib` |
| `macos-aarch64` | Mac OS X | aarch64 | `.dylib` |

## manifest.toml 规范

### 完整示例

```toml
# ──── 必填字段 ────

[package]
name = "example-mod"
version = "0.1.0"
description = "An example Morrow mod"
authors = ["dev <dev@example.com>"]
license = "MIT"

# ──── Morrow 特定字段 ────

[morrow]
api_version = 1                  # ABI API 版本
min_runtime = "0.1.0"           # 最低 Runtime 版本（semver）

# ──── Minecraft 兼容性 ────

[minecraft]
version = ">=1.20.1, <1.22"     # Minecraft 版本范围
loader = "fabric"               # 目标加载器

# ──── 入口 ────

[entry]
symbol = "morrow_mod_init"      # Rust extern "C" 入口函数名
# 或
# crate = "my_mod"              # 如果使用约定：自动推导符号名

# ──── 可选字段 ────

[build]
rustc_min = "1.80.0"           # 最低 Rust 版本

[config]                        # 默认配置（JSON）
greeting = "Hello from Morrow!"
max_entities = 1000

[dependencies]                  # Morrow mod 依赖
# other-mod = ">=1.0.0"
```

### 字段说明

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `package.name` | string | ✅ | Mod 唯一标识符（kebab-case） |
| `package.version` | string | ✅ | SemVer 版本 |
| `package.description` | string | ✅ | 一句话描述 |
| `package.authors` | string[] | ✅ | 作者列表 |
| `package.license` | string | ❌ | 许可证 |
| `morrow.api_version` | uint | ✅ | ABI 版本号 |
| `morrow.min_runtime` | string | ✅ | 最低 Runtime 版本 |
| `minecraft.version` | string | ✅ | MC 版本范围 |
| `minecraft.loader` | string | ✅ | 目标加载器 |
| `entry.symbol` | string | ✅ | 入口函数名 |
| `entry.crate` | string | ❌ | 替代 symbol 的约定 |
| `build.rustc_min` | string | ❌ | Rust 编译器最低版本 |
| `config.*` | any | ❌ | 默认配置键值对 |
| `dependencies.*` | string→string | ❌ | Mod 依赖和版本约束 |

## 平台选择算法

```rust
/// 给定当前平台，选择正确的 native artifact 路径
fn select_artifact(manifest: &Manifest, zip: &ZipArchive) -> Result<String> {
    let platform_id = format!(
        "{}-{}",
        std::env::consts::OS,        // "linux", "windows", "macos"
        std::env::consts::ARCH       // "x86_64", "aarch64"
    );

    // 标准化 OS 名称
    let platform_id = match platform_id.as_str() {
        "linux-x86_64"   => "linux-x86_64",
        "linux-aarch64"  => "linux-aarch64",
        "windows-x86_64" => "windows-x86_64",
        "macos-x86_64"   => "macos-x86_64",
        "macos-aarch64"  => "macos-aarch64",
        other => return Err(format!("Unsupported platform: {}", other)),
    };

    // 在 ZIP 中查找对应平台目录
    let dir = format!("{}/", platform_id);
    for entry in zip.file_names() {
        if entry.starts_with(&dir) {
            return Ok(entry.to_string());
        }
    }

    Err(format!("No artifact for platform: {}", platform_id))
}
```

## 构建流程

```bash
# 1. Rust mod 开发者构建
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-msvc

# 2. 打包
morrow-cli package ./my-mod

# 内部：
# - 读取 Cargo.toml → 自动生成大部分 manifest 字段
# - 收集 target/<triple>/release/*.{so,dll} 文件
# - 复制 assets/ 目录
# - 生成 manifest.toml
# - ZIP 打包（store 方法，不压缩）
```

## 加载流程

```
Java Host 侧:
1. Path modFile = Path.of("mods/my-mod.mor")
2. NativeModPackage pkg = ModPackageLoader.load(modFile)
3. pkg.validateManifest()           // 检查必填字段
4. pkg.checkCompatibility()          // 验证 api_version + mc version
5. Platform platform = Platform.detect()
6. String artifactPath = pkg.selectArtifact(platform)
7. Path extractedLib = pkg.extract(artifactPath, tempDir)
8. long modHandle = morrow_load_mod(runtimeHandle, extractedLib)
```

## 注意事项

- **不要压缩 native artifact** — 使用 ZIP store 方法，因为 .so/.dll 已经是 ELF/PE 格式，二次压缩无意义且拖慢加载
- **不嵌入 JAR** — .mor 是独立格式，不由 Java 类加载器管理
- **安全性** — v1 不验证签名，信任本地 mods/ 目录下的文件
- **大小限制** — 建议单个 .mor < 50MB（含所有平台 artifact）
