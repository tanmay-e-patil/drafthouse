FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev pkgconf

WORKDIR /app

COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl \
    -p ingress -p migrate-pg -p migrate-scylla \
    && rm -rf crates/*/src ingress/src dal/*/src nanoservices/*/networking/src nanoservices/*/core/src nanoservices/*/dal/src

COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl \
    -p ingress -p migrate-pg -p migrate-scylla


FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ingress /app/ingress
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/migrate-pg /app/migrate-pg
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/migrate-scylla /app/migrate-scylla
COPY migrations /app/migrations

ENTRYPOINT ["/app/ingress"]
