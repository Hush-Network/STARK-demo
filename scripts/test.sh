#!/bin/bash
set -e
cargo test --release "$@"
