#!/usr/bin/env bash

echo "Starting container..."

CONTAINTER_HASH=$(docker run \
    -e UID=1000 \
    -e GID=1000 \
    -v $(pwd)/.btc/:/home/bitcoin/.bitcoin \
    -p 18443:18443 \
    -d regtest \
    -regtest=1 \
    -printtoconsole \
    -prune=4096 \
    -rpcuser=dev \
    -rpcpassword=dev \
    -rpcallowip=0.0.0.0/0 \
    -rpcport=18443 \
    -rpcbind=0.0.0.0)

sleep 3 # We have to wait for the RPC to initialize

CARGO_TEST_ARG=$1

cargo test --locked --workspace -- --test-threads=1 $CARGO_TEST_ARG
docker stop $CONTAINTER_HASH > /dev/null

echo "Container stoped."
