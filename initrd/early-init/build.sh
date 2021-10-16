#!/bin/sh

NAME="early-init"

cd $(dirname $0)
[[ $1 = clean ]] && { cargo clean;  exit 0; }
[[ $1 = sysroot ]] && { ../gen-sysroot.sh; exit 0; }
[[ $1 = test ]] && { cargo test; exit 0; }
[[ $1 = fmt ]] && { cargo fmt; exit 0; }
[[ $1 = release ]] && RFLAG=--release

cargo build $RFLAG || exit 1

BIN=target/x86_64-os-userland/debug/$NAME
[[ $1 = release ]] && BIN=target/x86_64-os-userland/release/$NAME

cp $BIN $NAME.bin
