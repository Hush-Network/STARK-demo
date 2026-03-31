# Contributing

## Setup

This project requires Rust nightly. The pinned version is in `rust-toolchain.toml`.

```bash
rustup install nightly
rustup default nightly
```

## Development

```bash
scripts/test.sh          # run tests (50 tests, requires --release)
scripts/bench.sh         # run benchmarks
scripts/fmt.sh           # format code
cargo clippy -- -D warnings
```

## Pull requests

- Run `scripts/test.sh` and `cargo clippy -- -D warnings` before opening a PR.
- Keep commits focused. One logical change per commit.
- CI runs fmt check, clippy, and full test suite on every PR.
