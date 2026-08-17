# 00 — 概述：哲学、目标与非目标

## 为什么存在 Morrow

Minecraft 模组生态已经发展了十多年，几乎所有模组都是用 Java 写的。这没问题——Java 是 Minecraft 的原生语言，Forge/Fabric 生态极其成熟。

但有一个没有被很好满足的需求：**在 Minecraft 里运行原生代码，同时与 Java 模组生态共存。**

现有方案的问题：

| 方案 | 问题 |
|------|------|
| JNI 桥接 | 样板代码爆炸、性能损耗大、内存管理痛苦 |
| JNA | 更慢、反射开销、不适合高频调用 |
| 独立 native mod | 无法与 Java mod 通信、需要自研 loader |
| 纯 Rust 重写 Minecraft | 不现实、生态为零 |

Morrow 走一条中间路线：**Rust 写 Mod，Panama 做桥，Mixin 注入（v0.12 起独立于 Fabric）。**

## 核心设计原则

1. **First-class Rust experience** — Rust 开发者应该感觉自己在写 Rust，不是在写 Java FFI wrapper
2. **Safe by default** — Panic 隔离、类型安全 ABI、显式生命周期
3. **Zero overhead where it matters** — 热点路径（tick dispatch）零拷贝
4. **Gradual adoption** — 一个 mod 可以是 100% Rust，也可以是 Java + Rust 混合
5. **Explicit over magic** — 不搞全局状态、不搞隐式注入、capability 显式获取

## 目标用户画像

| 用户 | 需求 |
|------|------|
| Rust 开发者想给 MC 写 mod | 用熟悉的语言，不要学 Java |
| Java mod 开发者想加速热点 | Rust 重写性能敏感模块 |
| 服务器运维 | 低内存、低 CPU 占用、稳定不崩 |
| Modpack 作者 | 混合 Java + Rust mod，无冲突 |

## 非目标（明确说不做）

- **提供 Fabric API 兼容层** — Morrow 是独立 Loader + Platform，但不复刻 Fabric API 生态
- **100% Rust 重写 MC** — 不现实也不需要
- **自动 Java → Rust 翻译** — 不做，也不推荐
- **Web UI / 管理面板** — 不属于 Runtime 范畴
- **移动端 / Bedrock Edition** — 专注 Java Edition

## 项目命名

"Morrow" = 拉丁语 "铁"

- Iron → Morrow → Rust（双关：铁锈 = Rust）
- 暗示坚固、稳定、基础设施级别的可靠性
- 与 Minecraft 的 "铁锭" 有文化关联
