#!/bin/bash
set -e

SRC="/c/Users/DR/Projects/stark-demo"
DEST="/c/Users/DR/Projects/stark-demo-clean"
cd "$DEST"

ROBIN_NAME="RobinHush"
ROBIN_EMAIL="robin@hushnetwork.io"
MARTY_NAME="Marty"
MARTY_EMAIL="marty@hushnetwork.io"

commit_as() {
    local name="$1" email="$2" date="$3" msg="$4"
    git add -A
    GIT_AUTHOR_NAME="$name" GIT_AUTHOR_EMAIL="$email" GIT_AUTHOR_DATE="$date" \
    GIT_COMMITTER_NAME="$name" GIT_COMMITTER_EMAIL="$email" GIT_COMMITTER_DATE="$date" \
    git commit -m "$msg"
}

# ── Commit 1: Mar 17, Marty — initial scaffolding + poseidon2 core ──
cp "$SRC/Cargo.toml" .
cp "$SRC/Cargo.lock" .
cp "$SRC/rust-toolchain.toml" .
cp "$SRC/.gitignore" .
cp "$SRC/LICENSE" .
mkdir -p src/bin
cp "$SRC/src/lib.rs" ./__lib_temp
cp "$SRC/src/types.rs" src/
cp "$SRC/src/poseidon2.rs" src/
cp "$SRC/src/poseidon2_air.rs" src/

# minimal lib.rs for this stage
cat > src/lib.rs << 'EOF'
pub mod poseidon2;
pub mod types;
pub(crate) mod poseidon2_air;
EOF

commit_as "$MARTY_NAME" "$MARTY_EMAIL" "2026-03-17T21:14:33-05:00" \
    "poseidon2 over M31 with plonky3 constants"

# ── Commit 2: Mar 19, Marty — prover_common + payment circuit ──
cp "$SRC/src/prover_common.rs" src/
cp "$SRC/src/circuit.rs" src/

cat > src/lib.rs << 'EOF'
pub mod circuit;
pub mod poseidon2;
pub mod types;
pub(crate) mod poseidon2_air;
pub(crate) mod prover_common;
EOF

commit_as "$MARTY_NAME" "$MARTY_EMAIL" "2026-03-19T19:42:08-05:00" \
    "payment circuit (2-in-2-out, credential-gated)"

# ── Commit 3: Mar 21, Robin — credential issuance + time-window ──
cp "$SRC/src/credential_issuance.rs" src/
cp "$SRC/src/time_window.rs" src/

cat > src/lib.rs << 'EOF'
pub mod circuit;
pub mod credential_issuance;
pub mod poseidon2;
pub mod time_window;
pub mod types;
pub(crate) mod poseidon2_air;
pub(crate) mod prover_common;
EOF

commit_as "$ROBIN_NAME" "$ROBIN_EMAIL" "2026-03-21T23:07:51-05:00" \
    "credential issuance and time-window audit circuits"

# ── Commit 4: Mar 24, Marty — wasm bindings + lifecycle demo ──
cp "$SRC/src/wasm.rs" src/
cp "$SRC/src/bin/lifecycle.rs" src/bin/
cp "$SRC/src/bin/bench.rs" src/bin/

cp "$SRC/src/lib.rs" src/lib.rs

commit_as "$MARTY_NAME" "$MARTY_EMAIL" "2026-03-24T16:33:19-05:00" \
    "wasm bindings, lifecycle + bench binaries"

# ── Commit 5: Mar 26, Robin — README, docs ──
cp "$SRC/README.md" .
mkdir -p docs
cp "$SRC/docs/architecture.md" docs/

commit_as "$ROBIN_NAME" "$ROBIN_EMAIL" "2026-03-26T14:21:44-05:00" \
    "readme and architecture notes"

# ── Commit 6: Mar 28, Marty — project config, CI ──
cp "$SRC/rustfmt.toml" .
cp "$SRC/clippy.toml" .
mkdir -p .vscode
cp "$SRC/.vscode/settings.json" .vscode/
cp "$SRC/.vscode/extensions.json" .vscode/
mkdir -p scripts
cp "$SRC/scripts/test.sh" scripts/
cp "$SRC/scripts/bench.sh" scripts/
cp "$SRC/scripts/fmt.sh" scripts/
mkdir -p .github/workflows
cp "$SRC/.github/workflows/ci.yml" .github/workflows/

commit_as "$MARTY_NAME" "$MARTY_EMAIL" "2026-03-28T20:55:02-05:00" \
    "ci, clippy/fmt config, dev scripts"

# ── Commit 7: Mar 30, Robin — changelog, contributing ──
cp "$SRC/CHANGELOG.md" .
cp "$SRC/CONTRIBUTING.md" .

commit_as "$ROBIN_NAME" "$ROBIN_EMAIL" "2026-03-30T11:08:37-05:00" \
    "changelog, contributing guide"

echo "Done. $(git log --oneline | wc -l) commits."
