FROM rust:slim-bookworm

RUN apt update && \
    apt install -y libssl-dev ca-certificates pkg-config git lldb gdb vim nano curl && \
    rm -rf /var/lib/apt/lists/*

RUN rustup default stable

WORKDIR /workspace

CMD ["bash"]
