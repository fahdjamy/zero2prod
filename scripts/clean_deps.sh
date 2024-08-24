#!/bin/bash

# Check if cargo-udeps is installed
if ! command -v cargo-udeps &> /dev/null
then
    # Install cargo-udeps if not found
    echo "cargo-udeps is not installed. Installing..."
    cargo install cargo-udeps
fi

# Check if nightly toolchain is installed
if ! rustup toolchain list | grep -q nightly
then
    # Install nightly toolchain if not found
    echo "Nightly toolchain is not installed. Installing..."
    rustup toolchain install nightly
fi

# Run cargo-udeps to remove unused dependencies
echo "Removing unused all target [prd,dev,stg] dependencies..."
cargo +nightly udeps --all-targets

# to make this file executable, run cmd below in your terminal
# chmod +x scripts/clean_deps.sh
