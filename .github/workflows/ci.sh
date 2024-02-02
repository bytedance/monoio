#!/usr/bin/env sh

set -ex

CARGO=cargo
if [ "${CROSS}" = "1" ]; then
    export CARGO_NET_RETRY=5
    export CARGO_NET_TIMEOUT=10

    cargo install cross
    CARGO=cross
fi

# If a test crashes, we want to know which one it was.
export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1

# test monoio mod
cd "${PROJECT_DIR}"/monoio

"${CARGO}" test --target "${TARGET}"
"${CARGO}" test --target "${TARGET}" --release

"${CARGO}" test --target "${TARGET}" --all-features
"${CARGO}" test --target "${TARGET}" --all-features --release

# test monoio-compat mod
cd "${PROJECT_DIR}"/monoio-compat

"${CARGO}" test --target "${TARGET}"
"${CARGO}" test --target "${TARGET}" --release

"${CARGO}" test --target "${TARGET}" --all-features
"${CARGO}" test --target "${TARGET}" --all-features --release
