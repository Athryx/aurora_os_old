#!/bin/sh

cd $(dirname $0)

readelf -e iso/boot/kernel.bin > temp.txt
