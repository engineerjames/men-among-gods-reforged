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

WORKDIR /build
COPY . .
RUN cargo build --release -p server-utils

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

COPY --from=builder /build/target/release/map_viewer /usr/local/bin/
COPY --from=builder /build/target/release/template_viewer /usr/local/bin/
