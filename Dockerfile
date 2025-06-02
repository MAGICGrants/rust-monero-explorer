FROM rust:1.87-alpine AS builder

WORKDIR /build

COPY .git ./.git
COPY Cargo.toml Cargo.lock ./
COPY external ./external
COPY src ./src
RUN apk add musl-dev git
RUN cargo build --release

FROM alpine:3

COPY --from=builder /build/target/release/rust-monero-explorer /usr/local/bin
RUN addgroup -S explorer && adduser -S explorer -G explorer
USER explorer

EXPOSE 8081
ENTRYPOINT ["rust-monero-explorer"]