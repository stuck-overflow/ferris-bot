FROM rust:alpine AS builder

RUN apk add \
    build-base \
    openssl-dev

WORKDIR /usr/src/ferris-bot
COPY assets assets
COPY src src
COPY Cargo.toml .
RUN cargo build --release

FROM alpine
COPY --from=builder /usr/src/ferris-bot/target/release/twitch_queue_bot /usr/bin
ENTRYPOINT \
    /usr/bin/twitch_queue_bot \
    --config-file /config/ferrisbot.toml \
    --first-token-file /config/first-token.json

