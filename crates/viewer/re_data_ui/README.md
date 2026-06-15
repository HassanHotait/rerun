# re_data_ui

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_data_ui.svg)](https://crates.io/crates/re_data_ui)
[![Documentation](https://docs.rs/re_data_ui/badge.svg)](https://docs.rs/re_data_ui)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Provides UI elements for Rerun component data for the Rerun Viewer.

## Audio assets

This crate contains the selection-panel UI for logged component data, including audio assets.
`AssetAudio` stores audio bytes in a `Blob` plus an optional `MediaType`.
The viewer shows a play button for audio blobs and draws a waveform for uncompressed WAV assets.

## Required tools

- Rust toolchain matching the workspace `rust-version`.
- `cargo` and the platform C/C++ linker for native Rust builds.
- `pixi` for the standard Rerun task runner and dependency setup.
- FFmpeg on `PATH` if you also need the existing video asset UI.

## Rebuild

From the repository root:

```powershell
pixi run codegen
pixi run rs-fmt
cargo clippy -p re_data_ui --all-features
pixi run rerun-build
```

If `pixi` is not installed, install it first, then rerun the commands above.
After changing `.fbs` files, always run `pixi run codegen` before compiling so generated Rust, Python, C++, reflection, and docs stay in sync.
