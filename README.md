# vkrs

Name is subject to change whenever I think of a better one.

`vkrs` is a project where I am exploring both Rust and Vulkan for the first time.

My goal with it is to be able to build some sort of 3D environment with maybe some nice lighting
(perhaps raytracing if I ever decide to get hardware for it in the future), other fancy effects
(what though?), and with at least some level of physics built in.

## Building

``` sh
cargo build [--release]
```

## Running

``` sh
cargo run [--release]
```

Optionally with logging enabled

``` sh
RUST_LOG="vkrs=debug,vulkan=debug" cargo run [--release]
```

Where

- `vkrs` controls logs from the application itself, and
- `vulkan` controls logs from the Vulkan validation layers, this is only available in debug builds
  and has no effect in release builds.
