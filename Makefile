.PHONY: dev test build

dev:
	cargo run

test:
	cargo test

build:
	cargo build --release
