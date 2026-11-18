# cliffvm

The **cliffhanger VM**: a reactive graph of entities with getters and setters, written in Rust, with a SQLite store, an HTTP door and drivers for files, commands, HTTP, SQL and cron. Its packs and its pages are written in **c1f**, the field's own language ([chedim/cliffhanger](https://github.com/chedim/cliffhanger)). It is the engine of [babyseat](https://github.com/chedim/babyseat), the AI seat whose memory is a graph the model programs; it also builds for the browser (wasm) and talks to other programs over [GraphLink](https://github.com/chedim/graphlink).

**Status: the engine lives here.** This repository was cut from the `native/` folder of babyseat with its history, so `git log` shows the engine's life so far. Engine changes are made here and released here (a tag-triggered release builds the six platform binaries); babyseat takes the release named by its `CLIFFVM_VERSION`, and `native/` is gone from it. The tests that start the real seat on babyseat's own packs and apps live in babyseat (`seat-tests/`), where they run on the release (the library and the program).

## Build

```
cargo build --release --no-default-features --features bin     # the seat: target/release/cliffvm (and graphlink-bridge)
```

The browser build is `--no-default-features --features wasm` (`build-web.sh`, which writes to `../membrane/apps/barest.c1f/ui` next to this folder: babyseat's `scripts/build_wasm.sh` clones a tag, runs it and copies the three files).

License: MIT.

## Releases

The version is `version` in `Cargo.toml` (0.1.0 is the first on crates.io: this crate has its own line, apart from babyseat's). A release is: bump it, merge into `main`, then merge `main` into the branch `release`. `.github/workflows/publish.yml` then builds the binaries for every platform (`binaries.yml`), runs the tests, publishes `cliffvm` to crates.io and makes the GitHub release `v<version>` with the binaries attached (`cliffvm-<version>-<target>.tar.gz` and its `.sha256`: `bin/cliffvm`, `bin/graphlink-bridge`, `LICENSE`, `VERSION`, `MANIFEST.sha256`; macOS arm64 and x86_64, Linux glibc and musl on x86_64 and aarch64). A binary that does not build stops the run before the crates.io publish. A push to `release` whose version is already tagged publishes nothing. It needs the repository secret `CARGO_REGISTRY_TOKEN`. On every other push and on pull requests the same workflow runs `cargo publish --dry-run`: it packages and builds the crate as crates.io would receive it and uploads nothing. Publishing to crates.io cannot be undone (a version can be yanked, never deleted): creating the branch `release` the first time publishes the version `main` has then.
