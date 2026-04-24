FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev pkgconf

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/migrate-pg/Cargo.toml crates/migrate-pg/Cargo.toml
COPY crates/migrate-scylla/Cargo.toml crates/migrate-scylla/Cargo.toml
COPY ingress/Cargo.toml ingress/Cargo.toml
COPY crates/utils/Cargo.toml crates/utils/Cargo.toml
COPY crates/dal-tx-impl/Cargo.toml crates/dal-tx-impl/Cargo.toml
COPY crates/event-subscriber/Cargo.toml crates/event-subscriber/Cargo.toml
COPY crates/publish-event/Cargo.toml crates/publish-event/Cargo.toml
COPY dal/kernel/Cargo.toml dal/kernel/Cargo.toml
COPY dal/dal/Cargo.toml dal/dal/Cargo.toml
COPY nanoservices/auth/networking/Cargo.toml nanoservices/auth/networking/Cargo.toml
COPY nanoservices/auth/core/Cargo.toml nanoservices/auth/core/Cargo.toml
COPY nanoservices/auth/dal/Cargo.toml nanoservices/auth/dal/Cargo.toml
COPY nanoservices/documents/networking/Cargo.toml nanoservices/documents/networking/Cargo.toml
COPY nanoservices/documents/core/Cargo.toml nanoservices/documents/core/Cargo.toml
COPY nanoservices/documents/dal/Cargo.toml nanoservices/documents/dal/Cargo.toml
COPY nanoservices/collab/networking/Cargo.toml nanoservices/collab/networking/Cargo.toml
COPY nanoservices/collab/core/Cargo.toml nanoservices/collab/core/Cargo.toml
COPY nanoservices/collab/dal/Cargo.toml nanoservices/collab/dal/Cargo.toml

RUN mkdir -p crates/migrate-pg/src && echo "fn main() {}" > crates/migrate-pg/src/main.rs
RUN mkdir -p crates/migrate-scylla/src && echo "fn main() {}" > crates/migrate-scylla/src/main.rs
RUN mkdir -p ingress/src && echo "fn main() {}" > ingress/src/main.rs

RUN cargo build --release --target x86_64-unknown-linux-musl \
    -p ingress -p migrate-pg -p migrate-scylla \
    && rm -rf crates/*/src ingress/src

COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl \
    -p ingress -p migrate-pg -p migrate-scylla

RUN strip target/x86_64-unknown-linux-musl/release/ingress \
    && strip target/x86_64-unknown-linux-musl/release/migrate-pg \
    && strip target/x86_64-unknown-linux-musl/release/migrate-scylla

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ingress /app/ingress
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/migrate-pg /app/migrate-pg
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/migrate-scylla /app/migrate-scylla
COPY migrations /app/migrations

ENTRYPOINT ["/app/ingress"]
