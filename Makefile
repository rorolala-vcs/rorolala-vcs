CARGO ?= cargo

.PHONY: all check build clippy doc doc-open fmt test clean

all: check build

check:
	$(CARGO) check --workspace --all-targets --all-features

build:
	$(CARGO) build --workspace --all-features

clippy:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

doc:
	$(CARGO) doc --workspace --no-deps

doc-open:
	$(CARGO) doc --workspace --no-deps --open

fmt:
	$(CARGO) fmt --all

test:
	$(CARGO) test --workspace --all-features

clean:
	$(CARGO) clean
