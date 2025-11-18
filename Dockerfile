# Stage 1: Build the Rust project
FROM rust:slim-bookworm as builder

# Set the default toolchain to stable
RUN rustup default stable

# Create a directory for the project
WORKDIR /joel-bot

# Copy the cargo files and download dependencies
# This helps leverage Docker's layer caching
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

# Copy the source code
COPY ./src ./src

# Update packages and emsure necessary dependencies
RUN apt update && \
    apt install -y libssl-dev ca-certificates pkg-config && \
    rm -rf /var/lib/apt/lists/*

# Build the project in release mode
RUN cargo build --release

# Stage 2: Create a minimalistic image with the built binary
FROM debian:bookworm-slim

# Copy the binary from the builder stage
COPY --from=builder /joel-bot/target/release/joel-bot /joel-bot
# Copy configuration files
COPY config.yaml Rocket.toml /

# Update packages and install necessary dependencies
RUN apt update && \
    apt install -y libssl-dev ca-certificates pkg-config && \
    rm -rf /var/lib/apt/lists/*


# Set the Rocket environment to production and port to 8080
ENV ROCKET_ENV=production
ENV ROCKET_PORT=8080

# Expose port 8080
EXPOSE 8080

# Set the start command for the container
CMD ["/joel-bot"]