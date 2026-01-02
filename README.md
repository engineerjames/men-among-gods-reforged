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
cargo run --release --bin <client|server>
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
The client will eventually use [Bevy](https://bevyengine.org/) for its rendering and input handling. At the moment, the client is still a work in progress and doesn't really exist in any form.

# Server
The server is a command-line application that listens for incoming connections from clients. It is still in ALPHA stage and is not yet fully functional. You should be able to connect to it using any Merceneries of Astonia (v2) client, but expect bugs.

# Original Work
The original C code, graphics, and sound effects that were ported are based on the Mercenaries of Astonia (v2) engine by Daniel Brockhaus. Website: http://www.brockhaus.org/merc2.html

# Development Notes
Try not to judge the Rust code too harshly; I'm still learning the language! I also attempted (initially) to port the code structure exactly from C to Rust - which even the C code wasn't exactly the best. Refactoring will come in time.

# Getting Help
If you need help with the project, feel free to open an issue on GitHub or reach out to me directly at jamesleearmes@gmail.com.