# syntax=docker/dockerfile:1.7
FROM ubuntu:24.04 AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"
ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_TARGET_DIR=/workspace/target

WORKDIR /workspace

COPY . .

# BuildKit cache mounts make cargo's registry, git db, and target dir
# persist across builds, so we get true incremental compilation even when
# source files change. Binaries are copied out of the cached target dir
# within the same RUN since cache mounts are not part of the image layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/workspace/target,sharing=locked \
    cargo build --release -p server --bins \
    && mkdir -p /out \
    && cp target/release/server target/release/world-snapshot /out/

FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /out/server /app/server
COPY --from=builder /out/world-snapshot /app/world-snapshot
COPY server/assets/world_seed.wsnap /app/world_seed.wsnap

EXPOSE 5555
CMD ["/app/server"]
