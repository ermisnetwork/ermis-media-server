#!/bin/bash

# Check if target name and HTTP port are provided
if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Usage: ./media_single.sh <TARGET_NAME> <HTTP_PORT>"
    exit 1
fi

TARGET_NAME=$1
HTTP_PORT=$2

# Create target directory if it doesn't exist
mkdir -p "../target/${TARGET_NAME}"

export RUST_LOG="atm0s_sdn_network=error"
export RUST_BACKTRACE=1
export RUSTFLAGS="--cfg tokio_unstable"

cargo build --release --target-dir "../target/${TARGET_NAME}" && \
cargo run --release --target-dir "../target/${TARGET_NAME}" -- \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 1 \
    --workers 1 \
    --http-port ${HTTP_PORT} \
    media \
    --enable-token-api \
    --disable-gateway-agent \
    --disable-connector-agent
