#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

BOARD="${1:-opi5b}"
KERNEL_DIR="${KERNEL_DIR:-${ROOT_DIR}/kernel}"
FRAGMENT="${FRAGMENT:-${ROOT_DIR}/kernels/configs/rk3588_mainline_min.headless.fragment}"
KERNEL_REPO="${KERNEL_REPO:-https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git}"
BUILD_ROOT="${BUILD_ROOT:-${ROOT_DIR}/build/kernel}"

ensure_repo() {
	local dir="$1"
	local repo="$2"

	if [ -d "${dir}/.git" ]; then
		git -C "${dir}" fetch --tags
		git -C "${dir}" pull --ff-only
	elif [ -d "${dir}" ]; then
		rm -rf "${dir}"
		git clone "${repo}" "${dir}"
	else
		git clone "${repo}" "${dir}"
	fi
}

ensure_repo "${KERNEL_DIR}" "${KERNEL_REPO}"

cd "${KERNEL_DIR}"

	"${SCRIPT_DIR}/mkconfig-mainline.sh" "${KERNEL_DIR}" "${FRAGMENT}"

	# Build non-interactively; feed defaults if Kconfig unexpectedly prompts.
	(
		set +o pipefail
		yes "" | make -j"$(nproc)" ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image dtbs
	)

BOARD_OUT="${BUILD_ROOT}/${BOARD}"
rm -rf "${BOARD_OUT}"
mkdir -p "${BOARD_OUT}"

cp arch/arm64/boot/Image "${BOARD_OUT}/"
dtb_glob=(arch/arm64/boot/dts/rockchip/rk3588*.dtb)
if [[ -e "${dtb_glob[0]}" ]]; then
	cp "${dtb_glob[@]}" "${BOARD_OUT}/"
fi

[[ -f initrd.img ]] && cp initrd.img "${BOARD_OUT}/"

echo "Kernel artifacts copied to ${BOARD_OUT}"
