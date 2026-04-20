.PHONY: gen dev test build

gen:
	cargo run --bin ingress -- --generate-spec > openapi.json && cd frontend && pnpm openapi-ts -i ../openapi.json -o shared/api/generated

dev:
	trap 'kill 0' EXIT; cargo watch -x 'run --bin ingress' & cd frontend && pnpm dev & wait

test:
	cargo test --workspace && cd frontend && pnpm test

build:
	cargo build --release --workspace && cd frontend && pnpm build
