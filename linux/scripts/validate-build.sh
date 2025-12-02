#!/usr/bin/env bash
set -euo pipefail

########################################
# PATHS — WORK FROM scripts/ FOLDER
########################################

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# BUILD_ROOT is always ../build from scripts/
BUILD_ROOT="${SCRIPT_DIR}/../build"

BOARD="${BOARD:-opi5b}"
SOC="${SOC:-rockchip}"

UBOOT_DIR="${BUILD_ROOT}/uboot/${BOARD}"
KERNEL_DIR="${BUILD_ROOT}/kernel/${BOARD}"
ROOTFS_DIR="${BUILD_ROOT}/rootfs"
IMAGES_DIR="${BUILD_ROOT}/images"

IMG_NAME="${IMG_NAME:-puppyos-${BOARD}.img}"
DISK_IMG="${DISK_IMG:-${IMAGES_DIR}/${IMG_NAME}}"

IDBLOADER_IMG="${UBOOT_DIR}/idbloader.img"
UBOOT_ITB="${UBOOT_DIR}/u-boot.itb"

KERNEL_IMAGE="${KERNEL_DIR}/Image"
DTB_FILE="${KERNEL_DIR}/rk3588s-orangepi-5.dtb"

########################################
# COLORS
########################################

RED=$'\e[31m'
GRN=$'\e[32m'
YEL=$'\e[33m'
RST=$'\e[0m'

pass() { echo "${GRN}[OK]${RST}  $*"; }
warn() { echo "${YEL}[WARN]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*"; }

_have() { command -v "$1" >/dev/null 2>&1; }

########################################
# HELPERS
########################################

check_file_min_size() {
  local path="$1" min="$2" label="$3"
  if [[ ! -f "$path" ]]; then
    fail "$label missing: $path"
    return 1
  fi
  local sz; sz="$(stat -c%s "$path")"
  if (( sz < min )); then
    fail "$label too small ($sz bytes < $min)"
    return 1
  fi
  pass "$label OK ($sz bytes)"
}

check_rootfs() {
  if [[ ! -d "$ROOTFS_DIR" ]]; then
    fail "rootfs dir missing: $ROOTFS_DIR"
    return 1
  fi
  for d in etc bin usr lib; do
    [[ -d "$ROOTFS_DIR/$d" ]] || warn "rootfs missing: $d"
  done
  pass "rootfs structure OK"
}

check_u_boot_fit() {
  if ! _have dumpimage; then
    warn "dumpimage not installed — skipping deep FIT check"
    return 0
  fi
  if ! dumpimage -l "$UBOOT_ITB" >/dev/null 2>&1; then
    fail "u-boot.itb invalid FIT (dumpimage failed)"
    return 1
  fi
  pass "u-boot.itb FIT structure OK"
}

check_disk_image() {
  if [[ ! -f "$DISK_IMG" ]]; then
    warn "no disk image found: $DISK_IMG (skipping)"
    return 0
  fi

  pass "disk image found: $DISK_IMG"

  # check minimum size
  check_file_min_size "$DISK_IMG" 33554432 "disk image" || return 1

  # check partition table
  if _have fdisk; then
    fdisk -l "$DISK_IMG" | grep -q "W95 FAT32" || warn "no FAT32 partition detected"
    fdisk -l "$DISK_IMG" | grep -q "Linux" || warn "no Linux rootfs partition detected"
  fi

  # check Rockchip bootloader offsets
  if dd if="$DISK_IMG" bs=512 skip=64 count=16 | hexdump -C | grep -q '[1-9A-Fa-f]'; then
    pass "idbloader region (sector 64) non-zero"
  else
    warn "idbloader region appears empty"
  fi

  if dd if="$DISK_IMG" bs=512 skip=16384 count=16 | hexdump -C | grep -q '[1-9A-Fa-f]'; then
    pass "u-boot.itb region (sector 16384) non-zero"
  else
    warn "u-boot.itb region appears empty"
  fi
}

########################################
# MAIN
########################################

echo "== PuppyOS Build Validator =="
echo "BOARD=${BOARD}"
echo "SOC=${SOC}"
echo "BUILD_ROOT=${BUILD_ROOT}"
echo

overall_ok=1

echo "--- U-Boot ---"
check_file_min_size "$IDBLOADER_IMG" 65536 "idbloader.img" || overall_ok=0
check_file_min_size "$UBOOT_ITB"    262144 "u-boot.itb"     || overall_ok=0
check_u_boot_fit || overall_ok=0
echo

echo "--- Kernel ---"
check_file_min_size "$KERNEL_IMAGE" 2097152 "kernel Image" || overall_ok=0
check_file_min_size "$DTB_FILE"     32768   "DTB file"      || overall_ok=0
echo

echo "--- Rootfs ---"
check_rootfs || overall_ok=0
echo

echo "--- Disk Image (optional) ---"
check_disk_image || overall_ok=0
echo

if (( overall_ok )); then
  echo "${GRN}All checks passed!${RST}"
  exit 0
else
  echo "${RED}Some checks FAILED. Fix before flashing SD.${RST}"
  exit 1
fi
