#!/usr/bin/env bash
set -euo pipefail
KDIR="${1:-$PWD}"
FRAG="${2:-kernels/configs/rk3588_mainline_min.headless.fragment}"

cd "$KDIR"
make ARCH=arm64 defconfig
./scripts/kconfig/merge_config.sh -m .config "$FRAG"
make ARCH=arm64 olddefconfig
echo "Merged config written to $KDIR/.config"
