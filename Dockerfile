FROM rust:1.48-alpine3.11 as builder

RUN rustup override set nightly
RUN apk update --no-cache
RUN apk add musl-dev
RUN apk add libressl-dev

RUN mkdir -p /joel-bot

COPY Cargo.toml Cargo.lock /joel-bot/
COPY ./src /joel-bot/src
WORKDIR /joel-bot

# RUN cargo install
RUN cargo build --release

FROM scratch
COPY --from=builder /joel-bot/target/release/joel-bot /joel-bot
CMD ["/joel-bot"]
