#!/bin/sh

cd $(dirname $0)
[[ $1 = clean ]] && cargo clean && exit 0
[[ $1 = sysroot ]] && cargo sysroot && exit 0
[[ $1 = release ]] && RFLAG=--release

cargo build $RFLAG || exit 1

IMG=target/x86_64-os/debug/kernel
[[ $1 = release ]] && IMG=target/x86_64-os/release/kernel

cp $IMG iso/boot/kernel.bin

grub-mkrescue -d /usr/lib/grub/i386-pc -o kernel.iso iso 2> /dev/null