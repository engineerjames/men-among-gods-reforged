#!/usr/bin/env bash
set -euo pipefail

# Linux build dependencies (Bevy + SDL2).
# Safe to run in CI; intended for ubuntu-latest.

sudo apt-get update
sudo apt-get install -y \
  pkg-config \
  libasound2-dev \
  libudev-dev \
  libwayland-dev \
  libxkbcommon-dev \
  libx11-dev \
  libxi-dev \
  libxrandr-dev \
  libxcursor-dev \
  libxinerama-dev \
  libsdl2-dev \
  libsdl2-image-dev \
  libsdl2-mixer-dev \
  libsdl2-gfx-dev
