# ConfigDeck — run `make` to see every target.

.DEFAULT_GOAL := help
SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
MAKEFLAGS += --no-print-directory

CARGO := cd src-tauri && cargo
DEBUG_BIN := src-tauri/target/debug/configdeck
RELEASE_BIN := src-tauri/target/release/configdeck
BUNDLE_DIR := src-tauri/target/release/bundle

ifeq ($(NO_COLOR),)
B := \033[1m
D := \033[2m
C := \033[36m
G := \033[32m
Y := \033[33m
R := \033[31m
X := \033[0m
endif

# $(call step,Title) prints a section header before a recipe's commands.
step = @printf '\n$(C)▸$(X) $(B)%s$(X)\n' "$(1)"
done = @printf '$(G)✓ %s$(X)\n' "$(1)"

##@ Getting started

help: ## Show this help
	@printf '\n$(B)ConfigDeck$(X) $(D)— make <target>$(X)\n'
	@awk 'BEGIN {FS = ":.*## "} \
		/^##@/ { printf "\n$(B)%s$(X)\n", substr($$0, 5) } \
		/^[a-zA-Z0-9_-]+:.*## / { printf "  $(C)%-12s$(X) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@printf '\n$(D)Start with: make doctor && make setup && make dev$(X)\n\n'

doctor: ## Check toolchain and system libraries
	$(call step,Required)
	@fail=0; \
	chk() { if eval "$$2" >/dev/null 2>&1; then printf '  $(G)✓$(X) %s\n' "$$1"; \
		else printf '  $(R)✗$(X) %-22s $(D)%s$(X)\n' "$$1" "$$3"; fail=1; fi; }; \
	chk "Rust (cargo)"      "command -v cargo"                  "install via https://rustup.rs"; \
	chk "Node.js 20+"       "node -v | grep -qE '^v([2-9][0-9]|[1-9][0-9]{2})\\.'" "sudo pacman -S nodejs"; \
	chk "pnpm"              "command -v pnpm"                   "sudo pacman -S pnpm"; \
	chk "WebKitGTK 4.1"     "pkg-config --exists webkit2gtk-4.1" "sudo pacman -S webkit2gtk-4.1"; \
	chk "node_modules"      "test -d node_modules"              "make setup"; \
	printf '\n$(C)▸$(X) $(B)Real-app tests (make wdio)$(X)\n'; \
	chk "WebKitWebDriver"   "command -v WebKitWebDriver"        "ships with webkit2gtk-4.1"; \
	chk "tauri-driver"      "command -v tauri-driver || test -x $$HOME/.cargo/bin/tauri-driver" "make setup"; \
	chk "headless X server" "command -v xvfb-run || command -v Xvnc" "sudo pacman -S xorg-server-xvfb"; \
	echo; if [ $$fail = 0 ]; then printf '$(G)All good.$(X)\n'; else printf '$(Y)Fix the ✗ items above.$(X)\n'; exit 1; fi

setup: ## Install JS deps, Playwright's Chromium and tauri-driver
	$(call step,JavaScript dependencies)
	pnpm install --frozen-lockfile
	$(call step,Playwright Chromium)
	pnpm exec playwright install chromium
	$(call step,tauri-driver)
	@command -v tauri-driver >/dev/null || test -x $$HOME/.cargo/bin/tauri-driver \
		&& echo "already installed" || cargo install tauri-driver --locked --version 2.1.0
	$(call done,Setup complete — try: make dev)

##@ Develop

dev: ## Run the app with hot reload
	pnpm tauri dev

ui: ## Run the UI in a browser against the mock backend (localhost:1420)
	pnpm dev

run: ## Build (incremental) and run the release app
	$(call step,Release build)
	pnpm tauri build --no-bundle
	$(RELEASE_BIN)

##@ Test

test: test-rust test-ui ## Fast tests: Rust + UI unit tests
	$(call done,Fast tests passed)

test-rust: ## Rust unit and flow tests (also regenerates TS bindings)
	$(call step,Rust tests)
	$(CARGO) test --lib --tests -q

test-ui: ## Vitest component and store tests
	$(call step,UI tests)
	pnpm test

e2e: ## Playwright UI flows against the mock backend
	$(call step,Playwright)
	pnpm test:e2e

wdio: ## Real-app journeys (WebdriverIO) on a headless display
	$(call step,Debug app build)
	pnpm build:app
	$(call step,WebdriverIO)
	CONFIGDECK_BIN=$(DEBUG_BIN) scripts/headless.sh pnpm test:wdio

screenshots: ## Refresh docs/screenshots (3 widths x 2 themes)
	$(call step,Screenshots)
	UPDATE_SCREENSHOTS=1 pnpm exec playwright test tests/e2e/screenshots.spec.ts
	$(call done,Updated docs/screenshots)

check: lint test e2e wdio ## Everything CI runs, except bundling
	$(call done,All checks passed)

##@ Quality

lint: ## clippy, rustfmt check, TS typecheck, architecture rules
	$(call step,Architecture rules)
	pnpm check:boundaries
	$(call step,rustfmt)
	$(CARGO) fmt --check
	$(call step,clippy)
	$(CARGO) clippy --all-targets -q -- -D warnings
	$(call step,TypeScript)
	pnpm typecheck

fmt: ## Format Rust code
	$(CARGO) fmt

##@ Build

bundle: ## Build the AppImage and .deb
	$(call step,Release bundles)
	pnpm tauri build
	@printf '\n$(G)✓ Bundles:$(X)\n'; ls -1sh $(BUNDLE_DIR)/appimage/*.AppImage $(BUNDLE_DIR)/deb/*.deb | sed 's/^/  /'

##@ Package

aur: ## Build an Arch package from this checkout (AUR_ARGS=--dirty to include uncommitted files)
	scripts/aur-install.sh $(AUR_ARGS)

aur-install: ## Build that package and install it (sudo)
	scripts/aur-install.sh --install $(AUR_ARGS)

aur-deb: ## Package the local .deb as configdeck-bin, no Rust compile
	scripts/aur-install.sh --deb $(AUR_ARGS)

aur-srcinfo: ## Regenerate AUR .SRCINFO files
	scripts/aur-srcinfo.sh

clean: ## Remove build output (keeps node_modules)
	$(CARGO) clean
	rm -r dist test-results playwright-report packaging/build 2>/dev/null || true
	$(call done,Clean)

.PHONY: help doctor setup dev ui run test test-rust test-ui e2e wdio screenshots check lint fmt bundle aur aur-install aur-deb aur-srcinfo clean
