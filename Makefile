.PHONY: fmt lint test build smoke verify

fmt:
	cargo fmt --all --check

lint:
	cargo clippy --all-targets --all-features --locked -- -D warnings

test:
	cargo test --all-targets --all-features --locked

build:
	cargo build --release --locked

smoke:
	cargo run --quiet --locked -- audit --input examples/stable.tsv --baseline hg38 --format json > /dev/null
	cargo run --quiet --locked -- audit --input examples/reference_sensitive.tsv --baseline hg38 --output target/reference-sensitive.md

verify: fmt lint test build smoke
