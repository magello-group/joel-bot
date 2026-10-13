FROM rust:slim-trixie

RUN apt update && \
    apt install -y ca-certificates git lldb gdb vim nano curl && \
    rm -rf /var/lib/apt/lists/*

RUN rustup default stable && rustup component add clippy rustfmt

WORKDIR /workspace

CMD ["bash"]
