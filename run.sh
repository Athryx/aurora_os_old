#!/bin/sh

ISO_FILE="kernel/kernel.iso"
SUBDIRS="initfs kernel"

# first arg is builddir, second is out file, and third is build system arg
cd $(dirname $0)

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
	qemu-system-x86_64 -m 5120 -debugcon stdio -s -S $ISO_FILE & $TERM -e "/bin/gdb" "-x" "debug.gdb"
elif [[ -z $1 ]] || [[ $1 = release ]]
then
	qemu-system-x86_64 -m 5120 -cdrom $ISO_FILE -debugcon stdio
fi

#if [[ $3 = bochs ]]
#then
#	$TERM -e bochs -f bochsrc
#fi
