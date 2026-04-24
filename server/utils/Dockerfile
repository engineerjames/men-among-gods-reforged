# syntax=docker/dockerfile:1.7
# Stage 1: builder — use the official Rust image so cargo is pre-installed.
FROM rust:slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    libgl1-mesa-dev \
    libx11-dev \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    && rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_TARGET_DIR=/build/target

WORKDIR /build
COPY . .

# Persistent cache mounts for incremental cargo builds across `docker compose
# build` invocations. Binaries are copied out before the cache mount is
# detached so they end up in the runtime image.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release -p server-utils \
    && mkdir -p /out \
    && cp target/release/map_viewer target/release/template_viewer /out/

# Stage 2: runtime
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgl1 \
    libgl1-mesa-dri \
    libx11-6 \
    libxcb1 \
    libxcb-render0 \
    libxcb-shape0 \
    libxcb-xfixes0 \
    libxcursor1 \
    libxi6 \
    libxrandr2 \
    libxrender1 \
    libxext6 \
    && rm -rf /var/lib/apt/lists/*

# Use software rendering so the container works without a GPU.
ENV LIBGL_ALWAYS_SOFTWARE=1

COPY --from=builder /out/map_viewer /usr/local/bin/
COPY --from=builder /out/template_viewer /usr/local/bin/
