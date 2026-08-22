#!/bin/sh
set -e

# Mojang EULA — 运行服务端必须接受 (docker run -e EULA=true)
if [ "$EULA" = "true" ] || [ "$EULA" = "TRUE" ]; then
    mkdir -p run
    echo "eula=true" > run/eula.txt
fi

# 示例模组 → loom run 目录 (用户挂载 run/mods 时会整体覆盖)
mkdir -p run/mods
cp -f /opt/morrow/mods/*.mor run/mods/ || true

# loom runServer: 前台运行,下载好的依赖直接复用
exec ./gradlew --no-daemon runServer
