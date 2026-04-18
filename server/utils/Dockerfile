# Stage 1: builder
FROM ubuntu:24.04 AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    build-essential \
    libssl-dev \
    pkg-config \
    libgl1-mesa-dev \
    libx11-dev \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
    && rm -rf /var/lib/apt/lists/*

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /build
COPY . .
RUN cargo build --release -p server-utils

# Stage 2: runtime
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgl1-mesa-dri \
    libx11-6 \
    libxcb1 \
    libxcb-render0 \
    libxcb-shape0 \
    libxcb-xfixes0 \
    && rm -rf /var/lib/apt/lists/*

# Use software rendering so the container works without a GPU.
ENV LIBGL_ALWAYS_SOFTWARE=1

COPY --from=builder /build/target/release/map_viewer /usr/local/bin/
COPY --from=builder /build/target/release/template_viewer /usr/local/bin/
