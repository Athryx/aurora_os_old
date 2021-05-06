#!/bin/sh

cd $(dirname $0)
[[ $1 = clean ]] && cargo clean && exit 0
[[ $1 = sysroot ]] && cargo sysroot && exit 0
[[ $1 = release ]] && RFLAG=--release

cargo build $RFLAG || exit 1

BIN=target/x86_64-os/debug/kernel
[[ $1 = release ]] && BIN=target/x86_64-os/release/kernel

cp $BIN initfs.bin
