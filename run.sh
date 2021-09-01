#!/bin/sh

ISO_FILE="kernel/kernel.iso"
SUBDIRS="fs kernel"

cd $(dirname $0)

./include_initfs.sh

for SUBDIR in $SUBDIRS
do
	if ! $SUBDIR/build.sh $1
	then
		echo "$SUBDIR build failed"
		exit 1
	fi
done

if [[ $1 = debug ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S $ISO_FILE & $TERM -e "$HOME/.cargo/bin/rust-gdb" "-x" "debug.gdb"
elif [[ $1 = release ]] && [[ $2 = debug ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S $ISO_FILE & $TERM -e "$HOME/.cargo/bin/rust-gdb" "-x" "debug-release.gdb"
elif [[ $1 = bochs ]]
then
	$TERM -e bochs -f bochsrc
elif [[ -z $1 ]] || [[ $1 = release ]]
then
	qemu-system-x86_64 -m 5120 -cdrom $ISO_FILE -debugcon stdio
fi
