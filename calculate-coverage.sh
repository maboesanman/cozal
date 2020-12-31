#!/bin/bash

export LLVM_PROFILE_FILE="target/debug/coverage/cozal-%m.profraw"

# build the test binary with coverage instrumentation
executables=$(RUSTFLAGS="-Zinstrument-coverage" cargo test --tests --no-run --message-format=json | jq -r "select(.profile.test == true) | .executable")

# run instrumented tests
$executables

# combine profraw files
cargo profdata -- merge -sparse target/debug/coverage/cozal-*.profraw -o target/debug/coverage/cozal.profdata
# collect coverage
cargo cov -- export $executables \
    --instr-profile=target/debug/coverage/cozal.profdata \
    --format=lcov \
    --ignore-filename-regex="(.*\.cargo/registry/.*)|(.*\.rustup/.*)|(.*test.*)" \
    > target/debug/coverage/cozal.lcov
