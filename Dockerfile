FROM rust:1.48-alpine3.11 as builder

RUN rustup override set nightly
RUN apk update --no-cache
RUN apk add musl-dev
RUN apk add libressl-dev

RUN mkdir -p /joel

COPY Cargo.toml Cargo.lock /joel/
COPY ./src /joel/src
WORKDIR /joel

# RUN cargo install
RUN cargo build --release

FROM scratch
COPY --from=builder /joel/target/release/joel /joel
CMD ["/joel"]