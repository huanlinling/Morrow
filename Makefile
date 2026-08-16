# Morrow Development Commands
#
#   make build         → build everything
#   make test          → run all tests
#   make test-bridge   → Panama bridge tests (M0 + M1)
#   make package-hello → create hello-morrow.morrow
#   make clean         → remove build artifacts

.PHONY: build build-runtime build-mods test test-bridge package-hello clean

# ─── Build ──────────────────────────────────

build-runtime:
	@echo "==> Building Rust runtime..."
	cargo build --release

build-mods:
	@echo "==> Building example mods..."
	cargo build --release -p hello-morrow -p chat-bot -p motd

build: build-runtime build-mods
	@echo "==> Build complete."

# ─── Test ───────────────────────────────────

test:
	@echo "==> Running Rust tests..."
	cargo test

test-bridge:
	@echo "==> Running bridge tests..."
	cd bridge-java && bash build.sh

# ─── Package ─────────────────────────────────

package-hello: build-mods
	@echo "==> Packaging hello-morrow (+ chat-bot dependency)..."
	bash scripts/package-mod.sh examples/hello-morrow
	bash scripts/package-mod.sh examples/chat-bot
	bash scripts/package-mod.sh examples/motd

# ─── Clean ──────────────────────────────────

clean:
	@echo "==> Cleaning build artifacts..."
	cargo clean
	rm -rf bridge-java/out/
	rm -f *.morrow
