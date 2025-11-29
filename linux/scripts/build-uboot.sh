#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

BOARD="${1:-opi5b}"
UBOOT_DIR="${UBOOT_DIR:-${ROOT_DIR}/uboot}"
RKBIN_DIR="${RKBIN_DIR:-${ROOT_DIR}/rkbin}"
# keep build outputs under build/uboot/<board>
BUILD_ROOT="${BUILD_ROOT:-${ROOT_DIR}/build/uboot}"
DEFCONFIG="${DEFCONFIG:-}"
DEFAULT_ROCKCHIP_TPL="${RKBIN_DIR}/bin/rk35/rk3588_ddr_lp4_2112MHz_lp5_2400MHz_v1.19.bin"
DEFAULT_BL31="${RKBIN_DIR}/bin/rk35/rk3588_bl31_v1.51.elf"

sync_repo() {
	local repo_dir=$1
	local repo_url=$2

	if [ -d "${repo_dir}/.git" ]; then
		git -C "${repo_dir}" fetch --tags
		git -C "${repo_dir}" pull --ff-only
	else
		rm -rf "${repo_dir}"
		git clone "${repo_url}" "${repo_dir}"
	fi
}

sync_repo "${UBOOT_DIR}" https://source.denx.de/u-boot/u-boot.git
sync_repo "${RKBIN_DIR}" https://github.com/rockchip-linux/rkbin.git

pick_defconfig() {
	local candidates=(
		orangepi-5-rk3588s_defconfig
		orangepi-5-rk3588_defconfig
		orangepi-5-plus-rk3588_defconfig
		rock-5b-rk3588_defconfig
	)
	for c in "${candidates[@]}"; do
		if [[ -f "configs/${c}" ]]; then
			echo "$c"
			return 0
		fi
	done
	return 1
}

if [ -z "${ROCKCHIP_TPL:-}" ]; then
	if [ -f "${DEFAULT_ROCKCHIP_TPL}" ]; then
		export ROCKCHIP_TPL="${DEFAULT_ROCKCHIP_TPL}"
	else
		printf 'Missing ROCKCHIP_TPL. Set env var or place blob at %s\n' "${DEFAULT_ROCKCHIP_TPL}" >&2
		exit 1
	fi
fi

if [ -z "${BL31:-}" ]; then
	if [ -f "${DEFAULT_BL31}" ]; then
		export BL31="${DEFAULT_BL31}"
	else
		printf 'Missing BL31. Set env var or place blob at %s\n' "${DEFAULT_BL31}" >&2
		exit 1
	fi
fi

cd "${UBOOT_DIR}"

make distclean

if [[ -z "${DEFCONFIG}" ]]; then
	DEFCONFIG="$(pick_defconfig)" || { echo "ERROR: no suitable RK3588 OrangePi defconfig found in uboot/configs"; exit 1; }
fi

make CROSS_COMPILE=aarch64-linux-gnu- "${DEFCONFIG}"
make -j"$(nproc)" CROSS_COMPILE=aarch64-linux-gnu-

[[ -s idbloader.img ]] || { echo "ERROR: idbloader.img missing/empty"; exit 1; }
[[ -s u-boot.itb ]]    || { echo "ERROR: u-boot.itb missing/empty"; exit 1; }
itb_sz="$(stat -c%s u-boot.itb)"
if (( itb_sz < 262144 )); then
	echo "ERROR: u-boot.itb is suspiciously small (${itb_sz} bytes) — refusing."
	exit 1
fi

mkdir -p "${BUILD_ROOT}"
BOARD_OUT="${BUILD_ROOT}/${BOARD}"
rm -rf "${BOARD_OUT}"
mkdir -p "${BOARD_OUT}"
cp idbloader.img "${BOARD_OUT}/"
cp u-boot.itb "${BOARD_OUT}/"
cp u-boot.bin "${BOARD_OUT}/" >/dev/null 2>&1 || true

echo "U-Boot artifacts copied to ${BOARD_OUT}"
