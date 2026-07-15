# AGENTS.md

## Repo layout
- Single crate at `subtitler/` — all cargo commands run from there.
- Root has no `Cargo.toml`; it's just a git container.

## Build, test, format, lint
- `cargo build --verbose`  (no workspace, no Makefile)
- `cargo test --verbose`
- `cargo fmt -- --check` (2-space indent per `rustfmt.toml`, not Rust default 4)
- `cargo clippy -- -D warnings`

## Feature flags
- `default = ["http"]` — `parse_url` is gated behind `http` / `reqwest`.
- Examples needing HTTP: run with `cargo run --example parse-srt-http --features="http"`
- `--no-default-features` strips reqwest and all URL-parsing entrypoints.

## Architecture notes
- `src/lib.rs` is the library root; `src/main.rs` is the CLI binary.
- SRT (`srt.rs`) and VTT (`vtt.rs`) share: model (`model.rs`), timestamp utils (`utils.rs`), regex patterns (`config.rs`).
- Timestamps are **milliseconds** (`u64`), not seconds.
- `parse_url` functions compile only with `cfg(feature = "http")`.
- `generate()` functions **write** to files via `OpenOptions::write(true).truncate(true)` — they overwrite, not append.

## CLI
- `subtitler file <path>` / `subtitler url <url>`
- Format auto-detected by file extension or URL substring (`.srt` / `.vtt`).

## Testing
- Unit tests in `src/` plus integration tests in `tests/`.
- Run with `cargo test --all-targets`.
- Example `.srt` / `.vtt` fixture files live in `examples/`.

## CI
- `.github/workflows/rust.yml` — fmt, clippy, build/test with feature matrix
- `.github/workflows/release.yml` — cargo-dist release automation (triggered by git tags)
