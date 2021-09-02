#!/bin/sh

IMG="disk.img"
SUBDIRS="fs kernel"
KERNEL="kernel/kernel.bin"

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

./gen-img.sh $KERNEL $IMG

if [[ $1 = debug ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S $IMG & $TERM -e "$HOME/.cargo/bin/rust-gdb" "-x" "debug.gdb"
elif [[ $1 = release ]] && [[ $2 = debug ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S $IMG & $TERM -e "$HOME/.cargo/bin/rust-gdb" "-x" "debug-release.gdb"
elif [[ $1 = bochs ]]
then
	$TERM -e bochs -f bochsrc
elif [[ -z $1 ]] || [[ $1 = release ]]
then
	qemu-system-x86_64 -m 5120 -debugcon stdio $IMG
fi
