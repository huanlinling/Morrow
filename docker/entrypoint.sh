#!/bin/sh
set -e

# Mojang EULA — 运行服务端必须接受 (docker run -e EULA=true)
if [ "$EULA" = "true" ] || [ "$EULA" = "TRUE" ]; then
    echo "eula=true" > eula.txt
fi

if [ ! -f "$SERVER_JAR" ]; then
    echo "ERROR: Minecraft server JAR not found at $SERVER_JAR" >&2
    echo "Mount it: docker run -v /path/to/server.jar:$SERVER_JAR ..." >&2
    exit 1
fi

# shellcheck disable=SC2086 — JAVA_OPTS 需要按空格分词
exec java $JAVA_OPTS \
    -javaagent:/opt/morrow/morrow.jar \
    -jar "$SERVER_JAR" "$@"
