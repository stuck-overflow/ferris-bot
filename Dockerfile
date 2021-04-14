FROM rust:alpine AS builder

RUN apk add \
    build-base \
    openssl-dev

WORKDIR /usr/src/ferris_bot
COPY assets assets
COPY src src
COPY Cargo.toml .
RUN RUSTFLAGS="-C target-feature=-crt-static" cargo build --release
EXPOSE 10666

FROM alpine
COPY --from=builder /usr/src/ferris_bot/target/release/ferris_bot /usr/bin
RUN apk add libgcc
ENTRYPOINT \
    /usr/bin/ferris_bot \
    --config-file /config/ferrisbot.toml
