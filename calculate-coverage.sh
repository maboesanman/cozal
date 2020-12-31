#!/bin/bash

# build the test binary with coverage instrumentation
executable=$(RUSTFLAGS="-Zinstrument-coverage" cargo test --no-run --message-format=json | tail -n2 | head -n1 | jq -r .executable)

# run instrumented tests
$executable

# collect coverage
cargo profdata -- merge -sparse default.profraw -o default.profdata
cargo cov -- export $executable \
    --instr-profile=default.profdata \
    --format=lcov \
    --ignore-filename-regex="(.*\.cargo/registry/.*)|(.*\.rustup/.*)|(.*test.*)" \
    > default.lcov