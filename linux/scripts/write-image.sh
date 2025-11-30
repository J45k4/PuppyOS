#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

DISK_IMG="${1:-}"
DEV="${2:-}"
MIN_IMAGE_MB=64
IMAGE_SIZE=0

die() { echo "ERROR: $*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: sudo $(basename "$0") <image-file> <block-device>

Writes the disk image directly to the target devices after confirming it is removable,
not the running root disk, and that the image exists.
EOF
  exit 1
}

ensure_root() {
  [[ $EUID -eq 0 ]] || die "Run with sudo or as root"
}

ensure_args() {
  [[ -n "$DISK_IMG" && -n "$DEV" ]] || usage
}

ensure_image() {
  [[ -f "$DISK_IMG" ]] || die "Image file not found: $DISK_IMG"
}

ensure_block_device() {
  [[ -b "$DEV" ]] || die "$DEV is not a block device"
}

validate_image() {
  local size fdisk_output
  size="$(stat -c%s "$DISK_IMG")"
  (( size >= MIN_IMAGE_MB * 1024 * 1024 )) || die "Image size ${size}B is below ${MIN_IMAGE_MB}MB"
  if ! fdisk_output="$(fdisk -l "$DISK_IMG" 2>/dev/null)"; then
    die "Unable to read partition table from $DISK_IMG"
  fi
  IMAGE_SIZE="$size"
  echo ">> Image layout:"
  echo "$fdisk_output"
}

ensure_not_root_disk() {
  local root_src root_base
  root_src="$(findmnt -no SOURCE /)" || die "Failed to detect root filesystem source"
  root_base="${root_src%%[0-9]*}"
  root_base="${root_base%%p}"
  root_base="${root_base%%[0-9]*}"
  if [[ "$DEV" == "$root_base" ]]; then
    die "Refusing to operate on $DEV (appears to hold /: $root_src)"
  fi
}

ensure_removable() {
  local rm tran
  rm="$(lsblk -ndo RM "$DEV" 2>/dev/null | tr -d '[:space:]' || echo "")"
  tran="$(lsblk -ndo TRAN "$DEV" 2>/dev/null | tr -d '[:space:]' || echo "")"

  if [[ "$rm" != "1" ]]; then
    die "Device $DEV is not marked as removable (RM=$rm)"
  fi
  if [[ -n "$tran" && "$tran" != "usb" && "$tran" != "mmc" ]]; then
    die "Device $DEV transport is not USB/MMC (TRAN=$tran)"
  fi
}

confirm_action() {
  echo "About to ERASE ALL DATA on $DEV"
  lsblk "$DEV"
  echo
  read -r -p "Type YES to continue: " CONF
  [[ "$CONF" == "YES" ]] || die "Aborted by user."
}

unmount_partitions() {
  while read -r line; do
    local part="/dev/${line%% *}"
    umount "$part" 2>/dev/null || true
  done < <(lsblk -ln "$DEV" | awk 'NR>1 {print $1}')
}

write_image() {
  echo ">> Writing $DISK_IMG to $DEV"
  dd if="$DISK_IMG" of="$DEV" bs=4M status=progress conv=fsync
  sync
}

verify_write() {
  if [[ "$IMAGE_SIZE" -le 0 ]]; then
    die "Internal error: IMAGE_SIZE not initialized"
  fi
  echo ">> Verifying $DISK_IMG against $DEV"
  partprobe "$DEV" >/dev/null 2>&1 || true
  cmp -n "$IMAGE_SIZE" "$DISK_IMG" "$DEV" >/dev/null
  echo ">> Verification passed"
}

main() {
  ensure_root
  ensure_args
  ensure_image
  ensure_block_device
  validate_image
  ensure_not_root_disk
  ensure_removable
  confirm_action
  unmount_partitions
  write_image
  verify_write
  echo "Done. SD card should be ready."
}

main "$@"
