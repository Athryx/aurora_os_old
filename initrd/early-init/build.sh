#!/bin/sh

cd $(dirname $0)
[[ $1 = clean ]] && cargo clean && exit 0
[[ $1 = sysroot ]] && cargo sysroot && exit 0
[[ $1 = test ]] && cargo test && exit 0
[[ $1 = fmt ]] && cargo fmt && exit 0
[[ $1 = release ]] && RFLAG=--release

cargo build $RFLAG || exit 1

BIN=target/x86_64-os-userland/debug/early-init
[[ $1 = release ]] && BIN=target/x86_64-os-userland/release/early-init

cp $BIN early-init.bin
