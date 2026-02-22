#!/usr/bin/env bash
set -euo pipefail

# Linux build dependencies for SDL2 and the vcpkg bootstrap toolchain.
# SDL2 is sourced via cargo-vcpkg (statically linked) and does not need
# system SDL2 packages.
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
  autoconf \
  autoconf-archive \
  automake \
  libtool \
  cmake \
  libltdl-dev \
  ninja-build
