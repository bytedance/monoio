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

"${CARGO}" test --target "${TARGET}" --no-default-features
"${CARGO}" test --target "${TARGET}" --no-default-features --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features async-cancel
"${CARGO}" test --target "${TARGET}" --no-default-features --features async-cancel --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features zero-copy
"${CARGO}" test --target "${TARGET}" --no-default-features --features zero-copy --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features splice
"${CARGO}" test --target "${TARGET}" --no-default-features --features splice --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features macros
"${CARGO}" test --target "${TARGET}" --no-default-features --features macros --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features sync
"${CARGO}" test --target "${TARGET}" --no-default-features --features sync --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features utils
"${CARGO}" test --target "${TARGET}" --no-default-features --features utils --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features debug
"${CARGO}" test --target "${TARGET}" --no-default-features --features debug --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features legacy
"${CARGO}" test --target "${TARGET}" --no-default-features --features legacy --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features iouring
"${CARGO}" test --target "${TARGET}" --no-default-features --features iouring --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features tokio-compat
"${CARGO}" test --target "${TARGET}" --no-default-features --features tokio-compat --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features poll-io
"${CARGO}" test --target "${TARGET}" --no-default-features --features poll-io --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features signal
"${CARGO}" test --target "${TARGET}" --no-default-features --features signal --release

"${CARGO}" test --target "${TARGET}" --no-default-features --features signal-termination
"${CARGO}" test --target "${TARGET}" --no-default-features --features signal-termination --release

"${CARGO}" test --target "${TARGET}" --all-features
"${CARGO}" test --target "${TARGET}" --all-features --release
