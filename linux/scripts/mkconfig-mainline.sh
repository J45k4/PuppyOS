#!/usr/bin/env bash
set -euo pipefail
KDIR="${1:-$PWD}"
FRAG="${2:-kernels/configs/rk3588_mainline_min.hdmi.fragment}"

cd "$KDIR"
make ARCH=arm64 defconfig
./scripts/kconfig/merge_config.sh -m .config "$FRAG"

# Ensure noninteractive refinement of merged config by accepting defaults.
(
	set +e
	set +o pipefail
	yes "" | make -s ARCH=arm64 olddefconfig > /dev/null
	rc=$?
	exit $rc
)
echo "Merged config written to $KDIR/.config"
