#!/bin/sh

cd $(dirname $0)
[[ $1 = clean ]] && cargo clean && exit 0
[[ $1 = sysroot ]] && cargo sysroot && exit 0
[[ $1 = release ]] && RFLAG=--release

cargo build $RFLAG || exit 1

BIN=target/x86_64-os-userland-entry/debug/initfs
[[ $1 = release ]] && BIN=target/x86_64-os-userland-entry/release/initfs

cp $BIN initfs.bin
