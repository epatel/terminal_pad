# terminal_pad — common dev tasks.
# Cargo lives in ~/.cargo/bin; source the env so `make` works from any shell.
CARGO := . "$$HOME/.cargo/env" && cargo

.DEFAULT_GOAL := help

.PHONY: help run build release test check fmt fmt-check clippy lint clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

run: ## Run the app (debug build)
	$(CARGO) run

build: ## Build (debug)
	$(CARGO) build

release: ## Build optimized release binary
	$(CARGO) build --release

test: ## Run the test suite
	$(CARGO) test

check: ## Type-check without producing a binary
	$(CARGO) check

fmt: ## Format the code
	$(CARGO) fmt

fmt-check: ## Verify formatting (CI-friendly, no changes)
	$(CARGO) fmt --check

clippy: ## Lint with clippy, warnings as errors
	$(CARGO) clippy --all-targets -- -D warnings

lint: fmt-check clippy ## Run formatting check + clippy

clean: ## Remove build artifacts
	$(CARGO) clean
