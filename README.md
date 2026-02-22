[![Rust](https://github.com/engineerjames/men-among-gods-reforged/actions/workflows/rust.yml/badge.svg)](https://github.com/engineerjames/men-among-gods-reforged/actions/workflows/rust.yml)

# men-among-gods-reforged
Men Among Gods (MoA) client and server written in Rust. Both the client and server should be cross-platform; but have only been tested on macOS and Windows so far.

# Why Rust? 🦀
Apart from the obvious benefits - my main reason for using Rust was so I can learn!  Additionally, as my free time ebbs and flows, I wanted a language that would allow me to pick up and put down the project without too much mental overhead; particularly when it comes to dependency management.

# Building and Running
You will need to have [Rust](https://www.rust-lang.org/) and [Cargo
](https://doc.rust-lang.org/cargo/) installed on your system to build the project.

Once you have Rust and Cargo installed, simply clone the repository and run:

```bash
cargo build --release
```

This will build both the client and server in release mode. The binaries will then be located in the `target/release` directory. You can also run an application via:

```bash
# Only the server exists currently
cargo run --release --bin <men-among-gods-client|server>
```

## Profiling with Samply
To profile the server, I recommend using `samply` which can be installed via Cargo:
```bash
cargo install samply
```
Then you can run the server with `samply` like so:
```bash
samply record cargo run --bin server
```
This will generate a flamegraph that you can use to analyze the performance of the server.

# Client
The client uses [SDL2](https://www.libsdl.org/) via the [Rust SDL2 bindings](https://github.com/Rust-SDL2/rust-sdl2) for rendering, input handling, and audio.

The client is still in ALPHA stage and is not yet fully functional. Many features from the original Mercenaries of Astonia (v2) game are missing, and there are likely to be bugs. However, you should be able to connect to a server and explore the world to some extent.

## Building on Windows, macOS, and Linux

SDL2 is managed via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) on all platforms and linked statically — no system SDL2 installation required:

```bash
cargo install cargo-vcpkg
cargo vcpkg build --manifest-path client/Cargo.toml
cargo build
```

Linux builds additionally need system headers:
```bash
bash pipelines/install_linux_deps.sh
```

# Server
The server is a command-line application that listens for incoming connections from clients. It is still in ALPHA stage and is not yet fully functional. You should be able to connect to it using any Merceneries of Astonia (v2) client, but expect bugs.

# Original Work
The original C code, graphics, and sound effects that were ported are based on the Mercenaries of Astonia (v2) engine by Daniel Brockhaus. Website: http://www.brockhaus.org/merc2.html

# Music
There was no music in the original Mercenaries of Astonia (v2) game, so I have added some of my own compositions to the project. All music is original and created by James Armes (me). You can find the music files in the `client/assets/music` directory, and it can be disabled in the client settings.

# Development Notes
Try not to judge the Rust code too harshly; I'm still learning the language! I also attempted (initially) to port the code structure exactly from C to Rust - which even the C code wasn't exactly the best. Refactoring will come in time.

# Getting Help
If you need help with the project, feel free to open an issue on GitHub or reach out to me directly at jamesleearmes@gmail.com.