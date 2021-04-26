#!/bin/sh

# first arg is builddir, second is out file, and third is build system arg
cd $(dirname $0)
[[ $1 = release ]] && RFLAG=--release

#cargo bootimage $RFLAG || exit 1
cargo build $RFLAG || exit 1

#IMG=target/x86_64-os/debug/bootimage-rust_os.bin
#[[ $1 = release ]] && IMG=target/x86_64-os/release/bootimage-rust_os.bin

IMG=target/x86_64-os/debug/rust_os
[[ $1 = release ]] && IMG=target/x86_64-os/release/rust_os

cp $IMG iso/boot/kernel.bin

grub-mkrescue -d /usr/lib/grub/i386-pc -o kernel.iso iso 2> /dev/null

#if [[ $1 = debug ]]
#then
#	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S -drive format=raw,file=$IMG & $TERM -e "/bin/gdb" "-x" "debug.gdb"
#else
#	qemu-system-x86_64 -m 5120 -drive format=raw,file=$IMG -debugcon stdio
#fi

if [[ $1 = debug ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S kernel.iso & $TERM -e "/bin/gdb" "-x" "debug.gdb"
else
	qemu-system-x86_64 -m 5120 -cdrom kernel.iso -debugcon stdio
fi

#if [[ $3 = bochs ]]
#then
#	$TERM -e bochs -f bochsrc
#fi
