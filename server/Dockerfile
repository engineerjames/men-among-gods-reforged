FROM ubuntu:24.04 AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace

COPY . .
RUN cargo build --release -p server

FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/server /app/server
COPY --from=builder /workspace/server/assets/.dat /app/.dat

EXPOSE 5555
CMD ["/app/server"]
