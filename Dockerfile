# Stage 1: Build the Rust project
FROM rust:slim-trixie AS builder

# Create a directory for the project
WORKDIR /joel-bot

# Copy the cargo files and download dependencies
# This helps leverage Docker's layer caching
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

# Copy the source code
COPY ./src ./src

# Build the project in release mode.
# No extra build packages are needed: TLS uses rustls (no OpenSSL), and its
# crypto backend aws-lc-sys builds with the C compiler in rust:slim on
# x86_64/aarch64. Other targets may need cmake installed here.
RUN cargo build --release

# Stage 2: Create a minimalistic image with the built binary
FROM debian:trixie-slim

# Copy the binary from the builder stage
COPY --from=builder /joel-bot/target/release/joel-bot /joel-bot
# Copy configuration files
COPY config.yaml Rocket.toml /

# TLS uses rustls, only the root certificates are needed
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Set the Rocket environment to production and port to 8080
ENV ROCKET_ENV=production
ENV ROCKET_PORT=8080

# Expose port 8080
EXPOSE 8080

# Set entrypoint so arguments are forwarded to the binary
ENTRYPOINT ["/joel-bot"]