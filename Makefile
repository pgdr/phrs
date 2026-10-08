.PHONY: build fmt clippy release test deploy

.DEFAULT_GOAL := build

build:
	cargo build

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets

release:
	cargo build --release

test:
	cargo test

deploy:
	cargo publish --dry-run
