FROM --platform=$BUILDPLATFORM lukemathwalker/cargo-chef:latest-rust-1-alpine AS chef

ARG TARGETARCH

RUN apk add --no-cache build-base musl-dev pkgconf ca-certificates

WORKDIR /app

RUN case "$TARGETARCH" in \
        amd64) rustup target add x86_64-unknown-linux-musl ;; \
        arm64) rustup target add aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac

FROM chef AS planner

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cacher

COPY --from=planner /app/recipe.json recipe.json

RUN case "$TARGETARCH" in \
        amd64) export RUST_TARGET=x86_64-unknown-linux-musl ;; \
        arm64) export RUST_TARGET=aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && cargo chef cook --release --target "$RUST_TARGET" --recipe-path recipe.json

FROM chef AS builder

COPY . .
COPY --from=cacher /app/target target

RUN case "$TARGETARCH" in \
        amd64) export RUST_TARGET=x86_64-unknown-linux-musl ;; \
        arm64) export RUST_TARGET=aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && cargo build --release --target "$RUST_TARGET" \
        -p ingress -p migrate-pg -p migrate-scylla \
    && install -D "target/$RUST_TARGET/release/ingress" /out/app/ingress \
    && install -D "target/$RUST_TARGET/release/migrate-pg" /out/app/migrate-pg \
    && install -D "target/$RUST_TARGET/release/migrate-scylla" /out/app/migrate-scylla

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /out/app/ingress /app/ingress
COPY --from=builder /out/app/migrate-pg /app/migrate-pg
COPY --from=builder /out/app/migrate-scylla /app/migrate-scylla
COPY migrations /app/migrations

ENTRYPOINT ["/app/ingress"]
