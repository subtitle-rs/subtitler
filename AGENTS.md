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
- `subtitler parse <input>` — parse and display (input = path / URL / `-` for stdin)
- `subtitler convert <input> <output>` — format conversion (auto-detect source, infer target from extension)
- `subtitler validate <input>` — timing and quality checks (`--max-cps`, `--max-chars`, `--max-gap`)
- `subtitler edit <input> --output <out>` — sort / shift / merge / split / transform-fps
- `subtitler shift <input> <ms> --output <out>` — quick time shift (positive = delay)
- `subtitler normalize <input> --output <out>` — text normalization (`--all` or individual `--fix-ocr` / `--strip-hi` / `--quotes` / `--whitespace`)
- `subtitler quality <input>` — quality report (`--json`)
- `subtitler info <input>` — file statistics
- `subtitler detect <input>` — format detection only
- `subtitler pipeline <input> <output> --config ops.json` — declarative transformation pipeline (v2.0+)
- Format auto-detected by content signature; file extension / URL substring used as a hint.

## Testing
- Unit tests in `src/` plus integration tests in `tests/`.
- Run with `cargo test --all-targets`.
- Example `.srt` / `.vtt` fixture files live in `examples/`.

## CI
- `.github/workflows/rust.yml` — fmt, clippy, build/test with feature matrix
- `.github/workflows/release.yml` — cargo-dist release automation (triggered by git tags)
