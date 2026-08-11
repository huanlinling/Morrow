# Ferrum Development Commands
#
# TL;DR:
#   make build         → build everything
#   make test          → run all tests
#   make test-bridge   → M0/M1 Panama bridge tests
#   make package-hello → create hello-ferrum.ferrum
#   make clean         → remove build artifacts

.PHONY: build build-runtime build-mod test test-bridge package-hello clean

# ─── Build ──────────────────────────────────

build-runtime:
	@echo "==> Building Rust runtime..."
	cd runtime-rs && cargo build --release

build-mod:
	@echo "==> Building example mods..."
	cd examples/hello-ferrum && cargo build --release

build: build-runtime build-mod
	@echo "==> Build complete."

# ─── Test ───────────────────────────────────

test:
	@echo "==> Running Rust tests..."
	cd runtime-rs && cargo test
	cd sdk-rs && cargo test

test-bridge:
	@echo "==> Running Ferrum bridge tests..."
	cd bridge-java && bash build.sh

# ─── Package ─────────────────────────────────

package-hello: build-mod
	@echo "==> Packaging hello-ferrum..."
	bash scripts/package-mod.sh examples/hello-ferrum

# ─── Clean ──────────────────────────────────

clean:
	@echo "==> Cleaning build artifacts..."
	cd runtime-rs && cargo clean
	cd sdk-rs && cargo clean
	cd examples/hello-ferrum && cargo clean
	rm -rf bridge-java/out/
	rm -f *.ferrum
