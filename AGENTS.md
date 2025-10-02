# Repository Guidelines

## Project Structure & Module Organization
- `src/` — main code. Key modules: `binance/` (WS/REST clients), `market_data/`, `metrics/`, `cli/`, `config/`, `ui/`. Entry points: `main.rs`, `lib.rs`.
- `tests/` — integration tests (e.g., `integration_test.rs`).
- `docs/` — architecture and guides (e.g., `architecture.md`, `agent/sprint.md`).
- Config at repo root: `config.toml` (see `config.toml.example`).

## Build, Test, and Development Commands
- `make build` — release build (`cargo build --release`).
- `make run` — run binary in release mode.
- `make test` — run all tests (`cargo test`).
- `make fmt` — format with rustfmt.
- `cargo clippy --all-targets -- -D warnings` — lint; CI-grade.
- Examples: `RUST_LOG=info cargo run -- ui`, `cargo run -- demo`.

## Coding Style & Naming Conventions
- Formatting: rustfmt (4 spaces, max width 100, reordered imports). Run `make fmt` before PRs.
- Linting: clippy on all targets; treat warnings as errors in PRs.
- Naming: modules/files `snake_case`, types/traits `CamelCase`, functions/vars `snake_case`, constants `SCREAMING_SNAKE_CASE`.
- Errors: prefer `anyhow` for app flows and `thiserror` for library errors.
- Logs/metrics: use `tracing` and `metrics`; avoid `println!` in non-demo paths.

## Testing Guidelines
- Frameworks: `cargo test`, `tokio::test` for async, `wiremock` for HTTP, integration tests in `tests/`.
- Conventions: name files `*_test.rs` or add tests under `tests/`. Keep tests deterministic; mock network where possible.
- Commands: `cargo test -q`, single test: `cargo test path::to::test`.

## Commit & Pull Request Guidelines
- Commits: Conventional Commits (e.g., `feat: ...`, `refactor: ...`) per history.
- PRs: include a clear description, linked issues, reproduction steps, and screenshots/GIFs for TUI changes.
- Quality gate: pass `make fmt`, `cargo clippy -- -D warnings`, and `make test` locally.

## Security & Configuration Tips
- Do not commit secrets. Start from `config.toml.example` → `config.toml` and document non-defaults in the PR.
- Prefer environment variables for sensitive overrides (e.g., `RUST_LOG`, API endpoints).

## Agent-Specific Instructions
- Keep changes minimal and scoped; avoid unrelated refactors.
- Follow this file’s conventions for any code you touch.
- Prefer Makefile targets; update docs if commands change.
- When adding files, mirror existing module layout and naming.
