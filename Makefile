# Makefile for Knife project

.PHONY: help build run test clean fmt check dev install

# Default target
help:
	@echo "Available targets:"
	@echo "  build     - Build the project"
	@echo "  run       - Run the project"
	@echo "  test      - Run tests"
	@echo "  clean     - Clean build artifacts"
	@echo "  fmt       - Format code with rustfmt"
	@echo "  clippy    - Run clippy linter"
	@echo "  check     - Run cargo check"
	@echo "  dev       - Enter development shell"
	@echo "  install   - Install the binary"

build:
	cargo build

knife:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

fmt:
	cargo fmt

check:
	cargo check

dev:
	nix develop -c $$SHELL

install:
	cargo install --path .
