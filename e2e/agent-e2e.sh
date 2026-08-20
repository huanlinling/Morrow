#!/bin/bash
# Morrow agent-mode end-to-end suite against a real vanilla server.
#
# Usage: e2e/agent-e2e.sh [server-dir]
#   server-dir: a directory containing server.jar (1.20.1); when absent the
#   script bootstraps one under the given path (downloads Mojang's jar,
#   writes eula/server.properties, copies .morrow packages from the repo
#   root and bridge-java/build/libs/*-agent.jar).
#
# Covers: join, chat, death, leave (protocol-level fake client) and
# break/place (dig via fake client + place via the -Dmorrow.selftest.place
# self-test mixin). Exits non-zero on any missing event.
set -u
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_DIR="${1:-$REPO_ROOT/e2e/.server}"
SERVER_DIR="$(cd "$SERVER_DIR" 2>/dev/null && pwd || (mkdir -p "$SERVER_DIR" && cd "$SERVER_DIR" && pwd))"
AGENT_JAR="$REPO_ROOT/bridge-java/build/libs/morrow-host-1.20.1-1.0.1-agent.jar"
SERVER_JAR_URL="https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar"
FIFO=/tmp/morrow-e2e-in
LOG="$SERVER_DIR/e2e.log"

fail() { echo "FAIL: $1"; exit 1; }

[ -f "$AGENT_JAR" ] || fail "agent jar not built: $AGENT_JAR"
if [ ! -f "$SERVER_DIR/server.jar" ]; then
    echo "==> downloading server.jar"
    curl -sL -o "$SERVER_DIR/server.jar" "$SERVER_JAR_URL" || fail "download"
fi
# Hermetic run: a fresh world avoids persisted player abilities
# (creative sessions save invulnerable=true, which survives gamemode
# switches and silently neuters /kill and void damage).
rm -rf "$SERVER_DIR/world"
echo "eula=true" > "$SERVER_DIR/eula.txt"
cat > "$SERVER_DIR/server.properties" << 'EOF'
online-mode=false
gamemode=survival
spawn-protection=0
server-port=25565
EOF
mkdir -p "$SERVER_DIR/mods"
for pkg in hello-morrow chat-bot motd; do
    [ -f "$REPO_ROOT/$pkg.morrow" ] && cp "$REPO_ROOT/$pkg.morrow" "$SERVER_DIR/mods/" \
        && echo "==> mod: $pkg.morrow"
done

# Vanilla loads server.properties/eula/world from the WORKING DIRECTORY.
# The self-test flag makes our java uniquely identifiable for cleanup.
rm -f "$FIFO" && mkfifo "$FIFO"
(cd "$SERVER_DIR" && tail -f "$FIFO" | timeout 240 java -Dmorrow.selftest.place=true \
    --enable-preview --enable-native-access=ALL-UNNAMED \
    --add-opens java.base/java.net=ALL-UNNAMED -Xmx2G \
    -javaagent:"$AGENT_JAR" -jar server.jar nogui) \
    > "$LOG" 2>&1 &
cleanup() {
    for pid in $(pgrep -f "morrow.selftest.place=true" 2>/dev/null); do
        kill "$pid" 2>/dev/null
    done
    pkill -f "tail -f $FIFO" 2>/dev/null
    rm -f "$FIFO"
}
trap cleanup EXIT
for i in $(seq 1 60); do grep -q "Done (" "$LOG" 2>/dev/null && break; sleep 1; done
grep -q "Done (" "$LOG" || fail "server did not start"
echo "==> server up"

# ── phase 1: join + chat (the self-test mixin fires place AND break on
#    the first tick with a player; player-relative, terrain-independent) ──
python3 "$REPO_ROOT/e2e/fake_client.py" chat Steve "hi morrow" 20 &
CPID=$!
sleep 3
grep -q "+ Steve" "$LOG" || fail "join event"
grep -q "<Steve> hi morrow" "$LOG" || fail "chat event"
grep -q "place self-test: CONSUME" "$LOG" || fail "place self-test did not consume"
grep -q "Steve placed minecraft:dirt" "$LOG" || fail "place event"
grep -q "Steve broke minecraft:dirt" "$LOG" || fail "break event"

# ── phase 2: death (survival /kill — the recipe verified 2026-08-19) ──
echo "kill Steve" > "$FIFO"
for i in $(seq 1 24); do grep -q "Steve died" "$LOG" && break; sleep 0.5; done
grep -q "Steve died" "$LOG" || fail "death event"
wait $CPID
# the server logs the disconnect a moment after the client closes
for i in $(seq 1 20); do grep -q "\- Steve" "$LOG" && break; sleep 0.5; done
grep -q "\- Steve" "$LOG" || fail "leave event"

echo "stop" > "$FIFO"
sleep 3
echo "==> ALL E2E EVENTS VERIFIED"
