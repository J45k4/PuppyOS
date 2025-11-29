#!/usr/bin/env bash
set -euo pipefail

# Installs the packages listed in linux/README.md so the build helpers can run.
REQUIRED_PACKAGES=(
  debootstrap
  qemu-user-static
  binfmt-support
  yq
  flex
  bison
  parted
  kpartx
  rsync
  dosfstools
  e2fsprogs
  rkdeveloptool
  cloud-guest-utils
  swig
  libgnutls28-dev
  python3-pyelftools
)

echo "Updating apt cache..."
sudo apt-get update

echo "Installing PuppyOS build dependencies..."
sudo apt-get install -y "${REQUIRED_PACKAGES[@]}"

cat <<'EOF'
Dependency installation completed.

You still need:
  - aarch64-linux-gnu- toolchain (e.g., apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu)
  - Rockchip blobs in linux/rkbin/ (ROCKCHIP_TPL and BL31)
  - Kernel and U-Boot repositories in linux/kernel/ and linux/uboot/
EOF
