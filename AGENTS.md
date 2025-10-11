# Repository Guidelines

## Project Structure & Module Organization
- Core Rust sources live in `src/`, with key areas such as `binance/` (exchange clients), `market_data/`, `metrics/`, `cli/`, `config/`, and `ui/`; entry points are `src/main.rs` and `src/lib.rs`.
- Integration tests sit under `tests/` (e.g., `tests/integration_test.rs`) and expect mocked network boundaries.
- Architecture notes and sprint docs are in `docs/`; consult `docs/architecture.md` before large design changes.
- Repository-level configuration defaults are stored in `config.toml` (see `config.toml.example` when provisioning).

## Build, Test, and Development Commands
- `make build` compiles a release binary via `cargo build --release`.
- `make run` launches the release binary; add args such as `RUST_LOG=info make run -- ui` for specific targets.
- `make test` executes `cargo test` for unit and integration coverage.
- `make fmt` enforces `rustfmt` defaults; run before committing.
- `cargo clippy --all-targets -- -D warnings` performs lint checks aligned with CI.

## Coding Style & Naming Conventions
- Follow `rustfmt` (4-space indent, max width 100); rely on `make fmt` to normalize imports and layout.
- Use `snake_case` for modules/functions/variables, `CamelCase` for types/traits, and `SCREAMING_SNAKE_CASE` for constants.
- Prefer `anyhow` for application errors, `thiserror` for library error enums, and instrument runtime paths with `tracing` and `metrics`.

## Testing Guidelines
- Default to `cargo test -q`; narrow scope via `cargo test module::test_name` when iterating.
- Use `tokio::test` for async workflows and `wiremock` to isolate HTTP dependencies.
- Name test files `*_test.rs` or place integration scenarios under `tests/`; keep runs deterministic and free of external API calls.

## Commit & Pull Request Guidelines
- Adopt Conventional Commits (e.g., `feat: add ui metrics panel`, `fix: guard binance reconnect loop`).
- PRs must describe intent, link relevant issues, list reproduction steps, and include UI captures for TUI changes.
- Ensure `make fmt`, `cargo clippy -- -D warnings`, and `make test` pass locally before requesting review.

## Security & Configuration Tips
- Never commit secrets; derive custom settings from `config.toml.example` and document overrides.
- Prefer environment variables (e.g., `RUST_LOG`, API endpoints) for sensitive or environment-specific values.
- Review telemetry additions to avoid leaking PII.

## Agent Workflow Notes
- Keep changes minimal and scoped; avoid unrelated refactors or stylistic churn.
- Before editing files with user modifications, review diffs and coordinate rather than reverting.
- When sandboxed, request approvals only when necessary and default to project Make targets for tooling.
