#!/usr/bin/env bash
set -euo pipefail
OVERLAYS_DIR="${1:?overlays dir}"
CFG="${2:?distro yaml}"
OUT="${3:?output rootfs dir}"

# deps: debootstrap qemu-user-static binfmt-support yq
REL=$(yq -r '.release' "$CFG")
ARCH=$(yq -r '.arch' "$CFG")
MIRROR=$(yq -r '.mirror' "$CFG")

sudo rm -rf "$OUT"
sudo debootstrap \
  --arch="$ARCH" \
  --variant=minbase \
  --include="$(yq -r '.seed_packages | join(",")' "$CFG")" \
  "$REL" "$OUT" "$MIRROR"

sudo cp /usr/bin/qemu-aarch64-static "$OUT/usr/bin/" || true

sudo chroot "$OUT" bash -e <<'CHROOT'
set -e
echo "puppyos" > /etc/hostname
ln -sf /usr/share/zoneinfo/Etc/UTC /etc/localtime || true
echo "en_US.UTF-8 UTF-8" >> /etc/locale.gen
apt-get update
apt-get -y install locales
locale-gen en_US.UTF-8
update-locale LANG=en_US.UTF-8

# Disable snap & cloud-init if pulled in accidentally
systemctl disable --now snapd.socket snapd.service 2>/dev/null || true
apt-get -y purge snapd cloud-init || true
apt-get -y autoremove --purge
apt-get clean

# networkd + resolved
systemctl enable systemd-networkd systemd-resolved
ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf || true

# SSH hardening: prohibit-password by default; firstboot will create user
passwd -l root
sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config
CHROOT

# overlays
sudo rsync -aHAX "$OVERLAYS_DIR"/ "$OUT"/

# ensure firstboot runs
sudo chroot "$OUT" systemctl enable puppy-firstboot.service
