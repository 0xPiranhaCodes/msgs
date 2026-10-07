SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c

NAME := msgs
VERSION := $(shell awk -F'"' '/^version = / { print $$2; exit }' Cargo.toml)
ARM_TARGET := aarch64-apple-darwin
INTEL_TARGET := x86_64-apple-darwin
DIST_DIR := dist
PACKAGE := $(NAME)-$(VERSION)-macos-universal
ARCHIVE := $(DIST_DIR)/$(PACKAGE).tar.gz
CHECKSUM := $(ARCHIVE).sha256
PREFIX ?= $(HOME)/.local

.PHONY: help setup check build universal dist verify-dist install clean

help: ## Show available commands
	@printf "msgs build and distribution commands\n\n"
	@awk 'BEGIN {FS = ":.*## "}; /^[a-zA-Z0-9_-]+:.*## / {printf "  make %-13s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

setup: ## Install Rust targets required for a universal macOS binary
	command -v rustup >/dev/null || { echo "error: rustup is required" >&2; exit 1; }
	rustup target add $(ARM_TARGET) $(INTEL_TARGET)

check: ## Run formatting, linting, and tests
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test --all-targets

build: ## Build an optimized binary for the current Mac
	cargo build --release

universal: ## Build dist/msgs for both Apple Silicon and Intel Macs
	command -v lipo >/dev/null || { echo "error: Apple lipo is required (install Xcode Command Line Tools)" >&2; exit 1; }
	command -v strip >/dev/null || { echo "error: Apple strip is required (install Xcode Command Line Tools)" >&2; exit 1; }
	rustup target list --installed | grep -qx '$(ARM_TARGET)' || { echo "error: missing $(ARM_TARGET); run 'make setup'" >&2; exit 1; }
	rustup target list --installed | grep -qx '$(INTEL_TARGET)' || { echo "error: missing $(INTEL_TARGET); run 'make setup'" >&2; exit 1; }
	cargo build --release --target $(ARM_TARGET)
	cargo build --release --target $(INTEL_TARGET)
	mkdir -p $(DIST_DIR)
	lipo -create \
		target/$(ARM_TARGET)/release/$(NAME) \
		target/$(INTEL_TARGET)/release/$(NAME) \
		-output $(DIST_DIR)/$(NAME)
	strip -x $(DIST_DIR)/$(NAME)
	lipo $(DIST_DIR)/$(NAME) -verify_arch arm64 x86_64

dist: universal ## Create a versioned archive and SHA-256 checksum
	rm -rf $(DIST_DIR)/$(PACKAGE)
	mkdir -p $(DIST_DIR)/$(PACKAGE)
	install -m 0755 $(DIST_DIR)/$(NAME) $(DIST_DIR)/$(PACKAGE)/$(NAME)
	cp README.md INSTALL.md $(DIST_DIR)/$(PACKAGE)/
	tar -czf $(ARCHIVE) -C $(DIST_DIR) $(PACKAGE)
	cd $(DIST_DIR) && shasum -a 256 $(notdir $(ARCHIVE)) > $(notdir $(CHECKSUM))
	printf "Created %s\nCreated %s\n" "$(ARCHIVE)" "$(CHECKSUM)"

verify-dist: ## Verify checksum, architectures, and packaged CLI startup
	test -f $(ARCHIVE) || { echo "error: $(ARCHIVE) is missing; run 'make dist'" >&2; exit 1; }
	test -f $(CHECKSUM) || { echo "error: $(CHECKSUM) is missing; run 'make dist'" >&2; exit 1; }
	cd $(DIST_DIR) && shasum -a 256 -c $(notdir $(CHECKSUM))
	lipo $(DIST_DIR)/$(NAME) -verify_arch arm64 x86_64
	lipo -info $(DIST_DIR)/$(NAME)
	tmp_dir="$$(mktemp -d)"; trap 'rm -rf "$$tmp_dir"' EXIT; tar -xzf $(ARCHIVE) -C "$$tmp_dir"; "$$tmp_dir/$(PACKAGE)/$(NAME)" --help >/dev/null
	echo "Distribution verified: $(ARCHIVE)"

install: build ## Install the current-host binary under PREFIX/bin
	mkdir -p $(PREFIX)/bin
	install -m 0755 target/release/$(NAME) $(PREFIX)/bin/$(NAME)
	echo "Installed $(PREFIX)/bin/$(NAME)"

clean: ## Remove Cargo and distribution build output
	cargo clean
	rm -rf $(DIST_DIR)
