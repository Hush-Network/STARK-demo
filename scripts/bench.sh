#!/bin/bash
set -e
cargo run --bin bench --release "$@"
