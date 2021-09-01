#!/bin/sh

cd $(dirname $0)

IMG="disk.img"
DEV0="/dev/loop0"
DEV1="/dev/loop1"
F0=""
F1=""

echo "generating disk image..."

modprobe loop || exit 1

dd if=/dev/zero of=$IMG bs=512 count=131072 || exit 1

losetup $DEV0 $IMG || exit 1
F0="1"

cleanup () {
	if [ -d mnt ]
	then
		umount mnt
		rmdir mnt
	fi

	if [ -n $F0 ]
	then
		losetup -d $DEV0
	fi

	if [ -n $F1 ]
	then
		losetup -d $DEV1
	fi
}
trap cleanup EXIT

parted -s $DEV1 mklabel msdos mkpart primary ext2 1M 100% -a minimal set 1 boot on || exit 1

losetup $DEV0 $IMG -o 1048576 || exit 1
F1="1"

mke2fs $DEV1 || exit 1

mkdir -p mnt
mount $DEV1 mnt/ || exit 1

rm -rf mnt/*
cp -r -t mnt $1

grub-install --root-directory=mnt --no-floppy --target="i386-pc" --modules="normal part_msdos ext2 multiboot" $DEV0 || exit 1

echo "done"
exit 0
