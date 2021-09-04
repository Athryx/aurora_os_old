#!/bin/sh

SUBDIRS="early-init"

cd $(dirname $0)

for SUBDIR in $SUBDIRS
do
	if ! $SUBDIR/build.sh $1
	then
		echo "$SUBDIR build failed"
		exit 1
	fi
done

# temp
#cp early-init/early-init.bin initrd
gen-initrd --ahci ahci-server/ahci-server.bin --init early-init/early-init.bin --fs fs-server/fs-server.bin --ext2 ext2-server/ext2-server.bin --part-list part-list -o initrd
