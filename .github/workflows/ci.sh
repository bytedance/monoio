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

# only enable legacy driver
"${CARGO}" test --target "${TARGET}" --no-default-features --features "async-cancel,bytes,legacy,macros,utils"
"${CARGO}" test --target "${TARGET}" --no-default-features --features "async-cancel,bytes,legacy,macros,utils" --release

if [ "${TARGET}" = "x86_64-unknown-linux-gnu" ] || [ "${TARGET}" = "i686-unknown-linux-gnu" ]; then

    # only enabled uring driver
    "${CARGO}" test --target "${TARGET}" --no-default-features --features "async-cancel,bytes,iouring,macros,utils"
    "${CARGO}" test --target "${TARGET}" --no-default-features --features "async-cancel,bytes,iouring,macros,utils" --release

    # enable uring+legacy driver
    "${CARGO}" test --target "${TARGET}"
    "${CARGO}" test --target "${TARGET}" --release

    if [ "${CHANNEL}" == "nightly" ]; then
        "${CARGO}" test --target "${TARGET}" --all-features
        "${CARGO}" test --target "${TARGET}" --all-features --release
    fi

fi

# test monoio-compat mod
cd "${PROJECT_DIR}"/monoio-compat

"${CARGO}" test --target "${TARGET}"
"${CARGO}" test --target "${TARGET}" --release

if [ "${CHANNEL}" == "nightly" ]; then
    "${CARGO}" test --target "${TARGET}" --all-features
    "${CARGO}" test --target "${TARGET}" --all-features --release
fi
