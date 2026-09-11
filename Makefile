# Modern Makefile for bdd

.PHONY: all build release test lint fmt install clean

all: release

build:
	cargo build

release:
	cargo build --release
	ln -sf target/release/bdd ./bdd

test: release
	cargo test --all-targets --all-features

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

check-fmt:
	cargo fmt --check

install:
	cargo install --path .
	ln -sf ~/.cargo/bin/bdd ~/.local/bin/bdd

clean:
	cargo clean
	rm -f bdd
