# ─── Morrow — native Minecraft mod loader (Rust + Panama FFM) ────────────────
#
# 构建环境 (对应 README "Prerequisites"):
#   • JDK 21          — Java Agent + Panama FFM bridge (需 --enable-preview)
#   • Rust stable 1.80+ — runtime-rs (cdylib)、sdk-rs、示例模组
#   • Gradle 8.10     — wrapper 自动下载 (bridge-java, fabric-loom)
#   • Python 3        — scripts/package-mod.sh 打包 .mor
#   • make / bash / gcc — Makefile 编排 + Rust 链接器
#
# 用法:
#   # 测试 (默认目标): loom runServer 自动下载 Minecraft 1.20.1,无需手动挂 server.jar
#   docker build -t morrow-dev .
#   docker run --rm -it --memory=4g -e EULA=true morrow-dev
#   # 挂载自己的模组包 (覆盖内置示例):
#   docker run --rm -it --memory=4g -e EULA=true \
#     -v /path/to/mods:/morrow/bridge-java/run/mods \
#     morrow-dev
#
#   # 生产 (独立 Agent,不依赖 Fabric):
#   docker build --target runtime -t morrow-runtime .
#   docker run --rm -it \
#     -v /path/to/server.jar:/server/server.jar \
#     -v /path/to/mods:/server/mods \
#     -e EULA=true \
#     morrow-runtime
#
# 注意: Minecraft server.jar 受 Mojang 分发条款限制,生产镜像需自行挂载
#       1.20.1 服务端;首次构建需联网拉取 crates/Maven 依赖。

# ─── Stage 1: builder — 完整工具链 (JDK 21 + Rust + Gradle) ─────────────
FROM eclipse-temurin:21-jdk-noble AS builder

WORKDIR /morrow

# 系统依赖: gcc/链接器 (Rust)、make、python3 (打包脚本)、git/curl (rustup)
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        make \
        python3 \
        git \
        curl \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Rust stable (README 要求 1.80+;rustup 保持最新)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}" \
    CARGO_NET_RETRY=5 \
    CARGO_TERM_COLOR=always

# ── Rust 工作区: runtime + SDK + 示例模组 ──
COPY Cargo.toml Cargo.lock ./
COPY runtime-rs ./runtime-rs
COPY sdk-rs ./sdk-rs
COPY examples ./examples
RUN cargo build --release \
    && cargo test --release

# ── Java bridge: Agent JAR (wrapper 下载 Gradle 8.10,再拉 Minecraft 依赖) ──
COPY bridge-java ./bridge-java
COPY scripts ./scripts
COPY Makefile ./
RUN cd bridge-java && ./gradlew --no-daemon build

# ── 打包示例模组 + Panama 桥接测试 (M0/M1, 验证 .so 与 JVM 互操作) ──
# hello-morrow 依赖 chat-bot,三个都要打包
RUN make package-hello \
    && bash scripts/package-mod.sh examples/chat-bot \
    && bash scripts/package-mod.sh examples/motd \
    && make test-bridge

# ─── Stage 2: runtime — JRE 21 + Agent JAR + 原生库 (生产) ─────────────────
FROM eclipse-temurin:21-jre-noble AS runtime

# Agent JAR (processResources 已将 libmorrow_runtime.so 嵌入 natives/linux-x86_64/)
COPY --from=builder /morrow/bridge-java/build/libs/morrow-host-0.1.0.jar /opt/morrow/morrow.jar
# 兜底: 供 -Dmorrow.native.dir 或 java.library.path 检索 (jar 内已有一份)
COPY --from=builder /morrow/target/release/libmorrow_runtime.so /opt/morrow/natives/libmorrow_runtime.so
# 示例模组 (开发用; 生产请挂载自己的 mods/ 目录)
# hello-morrow 依赖 chat-bot,必须一起拷
COPY --from=builder /morrow/hello-morrow.mor /opt/morrow/mods/hello-morrow.mor
COPY --from=builder /morrow/chat-bot.mor /opt/morrow/mods/chat-bot.mor
COPY --from=builder /morrow/motd.mor /opt/morrow/mods/motd.mor

COPY docker/entrypoint.sh /opt/morrow/entrypoint.sh
RUN chmod +x /opt/morrow/entrypoint.sh \
    && mkdir -p /server/mods /server/world

WORKDIR /server
# Panama FFM 在 JDK 21 仍是 preview,必须带这两个 flag
ENV JAVA_OPTS="--enable-preview --enable-native-access=ALL-UNNAMED" \
    SERVER_JAR="/server/server.jar" \
    EULA=""

ENTRYPOINT ["/opt/morrow/entrypoint.sh"]

# ─── Stage 3: dev — 基于 builder 的 loom 测试镜像 (默认目标) ───────────────
# ./gradlew runServer: loom 自动下载 Minecraft 1.20.1 + Yarn 映射 + Fabric
# Loader dev 运行时,配好 -Dmorrow.native.dir 与 preview flags,即开即测。
FROM builder AS dev

# 示例模组包 (入口脚本会拷进服务端 cwd 的 mods/; hello-morrow 依赖 chat-bot)
COPY --from=builder /morrow/hello-morrow.mor /opt/morrow/mods/hello-morrow.mor
COPY --from=builder /morrow/chat-bot.mor /opt/morrow/mods/chat-bot.mor
COPY --from=builder /morrow/motd.mor /opt/morrow/mods/motd.mor

COPY docker/dev-entrypoint.sh /opt/morrow/dev-entrypoint.sh
RUN chmod +x /opt/morrow/dev-entrypoint.sh

# loom 的 runServer 必须在 bridge-java 项目目录里执行
WORKDIR /morrow/bridge-java
ENV EULA=""

ENTRYPOINT ["/opt/morrow/dev-entrypoint.sh"]
