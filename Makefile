# tmacs — build, test and release helpers.
# Run `make help` for the list of targets.

CARGO   ?= cargo
PREFIX  ?= $(HOME)/.local
BINDIR  ?= $(PREFIX)/bin
NAME    := tmacs
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TARGET  := $(shell rustc -vV | sed -n 's/^host: //p')
DISTDIR := dist
DIST    := $(DISTDIR)/$(NAME)-$(VERSION)-$(TARGET)

.DEFAULT_GOAL := help
.PHONY: help build release run test test-release bench lint fmt fmt-check check \
        install uninstall dist tag clean

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-13s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Debug build
	$(CARGO) build

release: ## Optimized build (target/release/tmacs)
	$(CARGO) build --release --locked

run: ## Run the debug build (ARGS="file ...")
	$(CARGO) run -- $(ARGS)

test: ## Run all unit and end-to-end tests
	$(CARGO) test

test-release: ## Run tests with optimizations
	$(CARGO) test --release

bench: ## Syntax highlighting throughput (BENCH_FILE=path/to/big.go)
	@test -n "$(BENCH_FILE)" || { echo "usage: make bench BENCH_FILE=<file>"; exit 1; }
	BENCH_FILE=$(BENCH_FILE) $(CARGO) test --release parse_speed -- --ignored --nocapture

lint: ## Clippy with warnings as errors
	$(CARGO) clippy --all-targets -- -D warnings

fmt: ## Format sources
	$(CARGO) fmt

fmt-check: ## Check formatting
	$(CARGO) fmt -- --check

check: fmt-check lint test ## Everything CI would run

install: release ## Install to $(BINDIR) (PREFIX=~/.local)
	install -d $(BINDIR)
	install -m 755 target/release/$(NAME) $(BINDIR)/$(NAME)
	@echo "installed $(BINDIR)/$(NAME) $(VERSION)"

uninstall: ## Remove the installed binary
	rm -f $(BINDIR)/$(NAME)

dist: check release ## Release tarball in dist/
	rm -rf $(DIST) && mkdir -p $(DIST)
	cp target/release/$(NAME) README.md $(DIST)/
	cp examples/init.toml $(DIST)/init.toml.example
	tar -C $(DISTDIR) -czf $(DIST).tar.gz $(notdir $(DIST))
	cd $(DISTDIR) && shasum -a 256 $(notdir $(DIST)).tar.gz > $(notdir $(DIST)).tar.gz.sha256
	@echo "$(DIST).tar.gz"

tag: check ## Create local git tag v$(VERSION)
	@git diff --quiet || { echo "working tree not clean"; exit 1; }
	git tag -a v$(VERSION) -m "$(NAME) $(VERSION)"

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf $(DISTDIR)
