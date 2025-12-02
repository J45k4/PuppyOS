#!/usr/bin/env bash
set -euo pipefail

OUT_IMG=""
ROOTFS=""
UBOOT_DIR=""
KERNEL_DIR=""
BOARD=""
BAUDRATE="${BAUDRATE:-1500000}"
EARLYCON="${EARLYCON:-1}"
EARLYCON_ADDR="${EARLYCON_ADDR:-0xfeb50000}"

# Ensure we have the privileges needed for loop/mount/mkfs; re-exec with sudo if not.
if [[ $EUID -ne 0 ]]; then
  exec sudo CONSOLE="${CONSOLE:-}" BOOTARGS_EXTRA="${BOOTARGS_EXTRA:-}" BAUDRATE="${BAUDRATE:-}" EARLYCON="${EARLYCON:-}" EARLYCON_ADDR="${EARLYCON_ADDR:-}" "$0" "$@"
fi

while [[ $# -gt 0 ]]; do
  case $1 in
    --output) OUT_IMG="$2"; shift 2;;
    --rootfs) ROOTFS="$2"; shift 2;;
    --uboot)  UBOOT_DIR="$2"; shift 2;;
    --kernel) KERNEL_DIR="$2"; shift 2;;
    --board)  BOARD="$2"; shift 2;;
    *) echo "Unknown arg $1"; exit 1;;
  esac
done

[[ -n "$OUT_IMG" && -n "$ROOTFS" && -n "$UBOOT_DIR" && -n "$KERNEL_DIR" ]] || { echo "missing args"; exit 1; }

BOOT_MB=256
ROOT_MB=4096
IMG_MB=$((BOOT_MB + ROOT_MB + 64))

dd if=/dev/zero of="$OUT_IMG" bs=1M count=$IMG_MB
parted -s "$OUT_IMG" mklabel gpt
parted -s "$OUT_IMG" mkpart boot fat32 1MiB ${BOOT_MB}MiB
parted -s "$OUT_IMG" mkpart root ext4 ${BOOT_MB}MiB 100%

LOOP=$(losetup -f --show "$OUT_IMG")
partprobe "$LOOP"
mkfs.vfat -F32 "${LOOP}p1"
mkfs.ext4 -F "${LOOP}p2"

mkdir -p /mnt/puppy-boot /mnt/puppy-root
mount "${LOOP}p1" /mnt/puppy-boot
mount "${LOOP}p2" /mnt/puppy-root

# rootfs
rsync -aHAX "$ROOTFS"/ /mnt/puppy-root/

# kernel
install -Dm0644 "$KERNEL_DIR/Image" /mnt/puppy-boot/Image
install -Dm0644 "$KERNEL_DIR/"*.dtb /mnt/puppy-boot/
# optional initrd
[[ -f "$KERNEL_DIR/initrd.img" ]] && install -Dm0644 "$KERNEL_DIR/initrd.img" /mnt/puppy-boot/

# simple extlinux (works with mainline u-boot)
mkdir -p /mnt/puppy-boot/extlinux
CONSOLE_DEFAULT="ttyS2,${BAUDRATE}n8"
EARLYCON_ARG=""
if [[ "${EARLYCON}" != "0" && -n "${EARLYCON}" ]]; then
	EARLYCON_ARG="earlycon=uart8250,mmio32,${EARLYCON_ADDR}"
fi
cat >/mnt/puppy-boot/extlinux/extlinux.conf <<EOF_INNER
timeout 1
default linux
menu title PuppyOS

label linux
  kernel /Image
  fdtdir /
  append console=${CONSOLE:-${CONSOLE_DEFAULT}} root=/dev/mmcblk0p2 rw rootwait ${EARLYCON_ARG} ${BOOTARGS_EXTRA}
EOF_INNER

sync

# u-boot (board prebuilts)
dd if="$UBOOT_DIR/idbloader.img" of="$LOOP" seek=64 conv=notrunc
dd if="$UBOOT_DIR/u-boot.itb"  of="$LOOP" seek=16384 conv=notrunc

umount /mnt/puppy-boot /mnt/puppy-root
losetup -d "$LOOP"
echo "Image ready: $OUT_IMG"
