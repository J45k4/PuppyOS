#!/usr/bin/env bash
set -euo pipefail

########################################
# USER CONFIG – adjust if you use non-default build outputs
########################################

SOC="${SOC:-rockchip}"          # change for your Orange Pi model
BOARD="${BOARD:-opi5b}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_ROOT="${BUILD_ROOT:-$(cd "${SCRIPT_DIR}/.." && pwd)/build}"

# Paths to bootloader files (from your U-Boot build)
IDBLOADER_IMG="${IDBLOADER_IMG:-${BUILD_ROOT}/uboot/${BOARD}/idbloader.img}"
UBOOT_ITB="${UBOOT_ITB:-${BUILD_ROOT}/uboot/${BOARD}/u-boot.itb}"
SUNXI_UBOOT="${SUNXI_UBOOT:-${BUILD_ROOT}/uboot/${BOARD}/u-boot-sunxi-with-spl.bin}"

# Kernel & DTB (from your kernel build)
KERNEL_IMAGE="${KERNEL_IMAGE:-${BUILD_ROOT}/kernel/${BOARD}/Image}"
DTB_FILE="${DTB_FILE:-${BUILD_ROOT}/kernel/${BOARD}/rk3588s-orangepi-5.dtb}"

# Root filesystem location (prefers directory, can fallback to tarball)
ROOTFS_DIR="${ROOTFS_DIR:-${BUILD_ROOT}/rootfs}"
ROOTFS_TAR="${ROOTFS_TAR:-}"

# Prebuilt disk image (optional): if set, the script writes this image directly
# and skips the manual partitioning/copy flow.
DISK_IMG="${DISK_IMG:-}"

# Kernel console
CONSOLE="${CONSOLE:-ttyS2,1500000}"   # Orange Pi 5 typical

# Boot partition size
BOOT_SIZE="${BOOT_SIZE:-200MiB}"

########################################

BOOT_MNT="/mnt/opiboot"
ROOT_MNT="/mnt/opiroot"

die() { echo "ERROR: $*" >&2; exit 1; }

########################################
# Parse argument
########################################

if [[ $# -ne 1 ]]; then
  echo "Usage: sudo $0 /dev/sdX"
  exit 1
fi

DEV="$1"

########################################
# Safety helpers
########################################

ensure_root() {
  [[ $EUID -eq 0 ]] || die "Run with sudo or as root"
}

ensure_block_device() {
  [[ -b "$DEV" ]] || die "$DEV is not a block device"
}

ensure_not_root_disk() {
  # Find device backing /
  local root_src root_base

  root_src="$(findmnt -no SOURCE /)" || die "Failed to detect root filesystem source"

  # Strip partition numbers: works for /dev/sda2 and /dev/nvme0n1p2, etc.
  root_base="${root_src}"
  root_base="${root_base%%[0-9]*}"      # /dev/sda2 -> /dev/sda, /dev/mmcblk0p2 -> /dev/mmcblk0p
  root_base="${root_base%%p}"           # /dev/mmcblk0p -> /dev/mmcblk0
  root_base="${root_base%%[0-9]*}"      # extra safety for weird names

  if [[ "$DEV" == "$root_base" ]]; then
    die "Refusing to operate on $DEV (appears to be the root disk: $root_src)"
  fi
}

ensure_removable_usb_or_mmc() {
  local rm tran
  rm="$(lsblk -ndo RM "$DEV" 2>/dev/null | tr -d '[:space:]' || echo "")"
  tran="$(lsblk -ndo TRAN "$DEV" 2>/dev/null | tr -d '[:space:]' || echo "")"

  # RM=1 => removable, RM=0 => non-removable
  if [[ "$rm" != "1" ]]; then
    die "Refusing to operate on $DEV: lsblk reports RM=$rm (0 = non-removable, 1 = removable)"
  fi

  # TRAN can be empty on some systems; only enforce if present
  if [[ -n "$tran" && "$tran" != "usb" && "$tran" != "mmc" ]]; then
    die "Refusing to operate on $DEV: transport '$tran' is not usb/mmc (got '$tran')"
  fi
}

check_files() {
  if [[ -n "$DISK_IMG" ]]; then
    [[ -f "$DISK_IMG" ]] || die "Disk image not found: $DISK_IMG"
    return
  fi

  [[ -f "$KERNEL_IMAGE" ]] || die "Kernel image not found: $KERNEL_IMAGE"
  [[ -f "$DTB_FILE" ]]     || die "DTB file not found: $DTB_FILE"
  if [[ -n "$ROOTFS_DIR" && -d "$ROOTFS_DIR" ]]; then
    :
  elif [[ -n "$ROOTFS_TAR" && -f "$ROOTFS_TAR" ]]; then
    :
  else
    die "Rootfs directory or tarball not found: ${ROOTFS_DIR} / ${ROOTFS_TAR}"
  fi

  case "$SOC" in
    rockchip)
      [[ -f "$IDBLOADER_IMG" ]] || die "Missing $IDBLOADER_IMG"
      [[ -f "$UBOOT_ITB" ]]     || die "Missing $UBOOT_ITB"
      itb_sz="$(stat -c%s "$UBOOT_ITB" 2>/dev/null || echo 0)"
      (( itb_sz >= 262144 )) || die "UBOOT_ITB too small (${itb_sz} bytes). This is almost certainly NOT a real U-Boot FIT."
      ;;
    sunxi)
      [[ -f "$SUNXI_UBOOT" ]] || die "Missing $SUNXI_UBOOT"
      ;;
    *)
      die "SOC must be rockchip or sunxi"
      ;;
  esac
}

confirm() {
  echo "About to ERASE ALL DATA on $DEV"
  lsblk "$DEV"
  echo
  read -r -p "Type YES to continue: " CONF
  [[ "$CONF" == "YES" ]] || die "Aborted by user."
}

unmount_partitions() {
  echo ">> Unmounting existing partitions on $DEV"
  while read -r line; do
    local part="/dev/${line%% *}"
    umount "$part" 2>/dev/null || true
  done < <(lsblk -ln "$DEV" | awk 'NR>1 {print $1}')
}

write_bootloader() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Writing U-Boot for $SOC to $DEV"
  case "$SOC" in
    rockchip)
      dd if="$IDBLOADER_IMG" of="$DEV" seek=64 conv=fsync status=progress
      dd if="$UBOOT_ITB"     of="$DEV" seek=16384 conv=fsync status=progress
      ;;
    sunxi)
      dd if="$SUNXI_UBOOT" of="$DEV" bs=1024 seek=8 conv=fsync status=progress
      ;;
  esac
}

partition_device() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Creating partition table on $DEV"
  parted -s "$DEV" mklabel msdos
  parted -s "$DEV" mkpart primary fat32 1MiB "$BOOT_SIZE"
  parted -s "$DEV" set 1 boot on
  parted -s "$DEV" mkpart primary ext4 "$BOOT_SIZE" 100%
  sleep 2
}

make_filesystems() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Creating filesystems"
  mkfs.vfat -n BOOT "${DEV}1"
  mkfs.ext4 -L rootfs "${DEV}2"
}

mount_partitions() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Mounting partitions"
  mkdir -p "$BOOT_MNT" "$ROOT_MNT"
  mount "${DEV}1" "$BOOT_MNT"
  mount "${DEV}2" "$ROOT_MNT"
}

install_kernel_and_dtb() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Installing kernel and DTB"
  cp "$KERNEL_IMAGE" "$BOOT_MNT/Image"

  mkdir -p "$BOOT_MNT/dtbs"
  cp "$DTB_FILE" "$BOOT_MNT/dtbs/"
  local dtb_base
  dtb_base=$(basename "$DTB_FILE")

  mkdir -p "$BOOT_MNT/extlinux"
  cat > "$BOOT_MNT/extlinux/extlinux.conf" <<EOF
DEFAULT primary
TIMEOUT 3

LABEL primary
    KERNEL /Image
    FDT /dtbs/$dtb_base
    APPEND root=LABEL=rootfs rw console=$CONSOLE earlycon
EOF
}

install_rootfs() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Installing root filesystem"
  if [[ -n "$ROOTFS_DIR" && -d "$ROOTFS_DIR" ]]; then
    rsync -aHAX "$ROOTFS_DIR"/ "$ROOT_MNT"/
  else
    echo ">> Using tarball $ROOTFS_TAR"
    tar xpf "$ROOTFS_TAR" -C "$ROOT_MNT"
  fi

  [[ -e "$ROOT_MNT/dev/console" ]] || mknod -m 600 "$ROOT_MNT/dev/console" c 5 1
  [[ -e "$ROOT_MNT/dev/null" ]]    || mknod -m 666 "$ROOT_MNT/dev/null" c 1 3
}

cleanup() {
  [[ -n "$DISK_IMG" ]] && return
  echo ">> Syncing and unmounting"
  sync
  umount "$BOOT_MNT" || true
  umount "$ROOT_MNT" || true
  echo "Done. SD card is ready."
}

write_full_image() {
  echo ">> Writing disk image $DISK_IMG to $DEV"
  dd if="$DISK_IMG" of="$DEV" bs=4M status=progress conv=fsync
  sync
  echo "Done. SD card is ready."
}

########################################
# MAIN
########################################

ensure_root
ensure_block_device
ensure_not_root_disk
ensure_removable_usb_or_mmc
check_files
confirm
if [[ -n "$DISK_IMG" ]]; then
  unmount_partitions
  write_full_image
else
  unmount_partitions
  write_bootloader
  partition_device
  make_filesystems
  mount_partitions
  install_kernel_and_dtb
  install_rootfs
  cleanup
fi
