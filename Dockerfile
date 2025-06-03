FROM rust:1.87-alpine AS builder

WORKDIR /build

COPY .git ./.git
COPY Cargo.toml Cargo.lock ./
COPY external ./external
COPY src ./src
RUN apk add musl-dev git curl
RUN cargo build --release

FROM alpine:3

COPY --from=builder /build/target/release/rust-monero-explorer-api /usr/local/bin
RUN addgroup -S cuprate && adduser -S cuprate -G cuprate
USER cuprate

EXPOSE 8081
ENTRYPOINT ["rust-monero-explorer-api"]
CMD ["-i", "0.0.0.0"]