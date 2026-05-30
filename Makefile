.PHONY: help
help: ## Ask for help!
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; \
		{printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: build
build: ## Build the project in debug mode
	cargo build

.PHONY: build-release
build-release: ## Build the project in release mode
	cargo build --release

.PHONY: check
check: ## Check code for compilation errors
	cargo check --all-targets

.PHONY: check-format
check-format: ## Check code formatting
	cargo +nightly fmt -- --check

.PHONY: format
format: ## Format code
	cargo +nightly fmt

.PHONY: lint
lint: ## Run linter
	cargo clippy --all-targets -- -D warnings

.PHONY: test
test: ## Run tests
	cargo test

.PHONY: example
example: ## Generate the example plugin into examples/out/
	cargo run --example hello

.PHONY: verify-elisp
verify-elisp: example ## Byte-compile and load the generated plugin in Emacs
	cd examples/out && \
		emacs --batch --eval '(setq byte-compile-error-on-warn t)' \
			-f batch-byte-compile ferrel-hello.el && \
		emacs --batch -l ./ferrel-hello.el \
			--eval '(princ (format "ok: %s\n" (ferrel-hello-double-sum 2 3)))' && \
		rm -f ferrel-hello.elc

.PHONY: transpile-example
transpile-example: ## Transpile examples/sample_config.rs to examples/out/
	cargo run --bin ferrel-transpile -- \
		examples/sample_config.rs -o examples/out/sample-config.el

.PHONY: verify-transpile
verify-transpile: ## Transpile the sample config and byte-compile it cleanly
	EMACS=$${EMACS:-emacs} cargo run --bin ferrel-transpile -- \
		examples/sample_config.rs -o examples/out/sample-config.el \
		--byte-compile && \
		rm -f examples/out/sample-config.elc

.PHONY: corpus-fetch
corpus-fetch: ## Download a random MELPA .el corpus into corpus/ (COUNT=50)
	COUNT=$${COUNT:-50} OUTDIR=corpus scripts/fetch-melpa-corpus.sh

.PHONY: corpus-test
corpus-test: ## Parse the downloaded corpus (no eval) and report failures
	cargo run --release --example corpus -- corpus

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean
	rm -f examples/out/*.elc
	rm -rf corpus corpus-report.md

.PHONY: setup
setup: ## Setup development environment
	rustup component add rustfmt clippy

# --- Documentation site ------------------------------------------------------

.PHONY: doc-install
doc-install: ## Install documentation dependencies
	npm --prefix doc install

.PHONY: doc-dev
doc-dev: ## Run documentation dev server
	(cd doc && npx docusaurus start)

.PHONY: doc-build
doc-build: ## Build documentation for production
	(cd doc && npx docusaurus build)

.PHONY: doc-serve
doc-serve: ## Serve built documentation locally
	(cd doc && npx docusaurus serve)

.PHONY: doc-clear
doc-clear: ## Clear documentation cache
	(cd doc && npx docusaurus clear)
