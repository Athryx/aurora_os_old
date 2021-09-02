#!/bin/sh

#boot directory with kernel.bin and grub.cfg is passed in argument 1

cd $(dirname $0)

IMG="disk.img"
PART_NUM="p1"
DEV0="/dev/loop0"
LOOP=""
MNT=""

echo "generating disk image..."

sudo modprobe loop || exit 1

rm -f $IMG

dd if=/dev/zero of=$IMG bs=512 count=131072 || exit 1

sudo losetup $DEV0 $IMG || exit 1
LOOP="1"

cleanup () {
	if [ -n $MNT ]
	then
		sudo umount /mnt || ( sleep 1 && sync && sudo umount /mnt )
	fi

	if [ -n $LOOP ]
	then
		sudo losetup -d $DEV0
	fi
}
trap cleanup EXIT

sudo parted -s $DEV0 mklabel msdos mkpart primary ext2 1M 100% -a minimal set 1 boot on || exit 1

sudo mke2fs $DEV0$PART_NUM || exit 1

#mkdir -p mnt
#sudo mount $DEV0$PART_NUM mnt/ || exit 1

#sudo rm -rf mnt/boot/
#sudo cp -r $1 mnt/boot

sudo mount $DEV0$PART_NUM /mnt || exit 1
MNT="1"

sudo rm -rf /mnt/boot/
sudo cp -r $1 /mnt/boot

sudo grub-install --root-directory=/mnt --no-floppy --target="i386-pc" --modules="normal part_msdos ext2 multiboot" $DEV0 || exit 1

echo "done"
exit 0