# Contributing

## Setup

```
rustup install nightly
rustup default nightly
```

## Development

```bash
scripts/test.sh          # run tests
scripts/bench.sh         # run benchmarks
scripts/fmt.sh           # format code
cargo clippy -- -D warnings
```

## Pull requests

- Run `scripts/test.sh` and `cargo clippy` before opening a PR.
- Keep commits focused. One logical change per commit.
