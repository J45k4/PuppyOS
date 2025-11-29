#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OVERLAYS_DIR="${1:?overlays dir}"
CFG="${2:?distro yaml}"
OUT="${3:-${ROOT_DIR}/build/rootfs}"

# deps: debootstrap qemu-user-static binfmt-support yq
REL=$(yq -r '.release' "$CFG")
ARCH=$(yq -r '.arch' "$CFG")
MIRROR=$(yq -r '.mirror' "$CFG")

sudo rm -rf "$OUT"
sudo mkdir -p "$OUT"
sudo debootstrap \
  --arch="$ARCH" \
  --variant=minbase \
  --include="$(yq -r '.seed_packages | join(",")' "$CFG")" \
  "$REL" "$OUT" "$MIRROR"

sudo cp /usr/bin/qemu-aarch64-static "$OUT/usr/bin/" || true

mount_binds() {
	sudo mount --bind /proc "$OUT/proc"
	sudo mount --bind /sys "$OUT/sys"
	sudo mount --bind /dev "$OUT/dev"
	sudo mount --bind /dev/pts "$OUT/dev/pts"
}

umount_binds() {
	for target in "$OUT/dev/pts" "$OUT/dev" "$OUT/sys" "$OUT/proc"; do
		sudo umount "$target" 2>/dev/null || true
	done
}

mount_binds
trap umount_binds EXIT

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

# networkd + resolved (enable only if present)
systemctl enable systemd-networkd || true
if [ -f /lib/systemd/system/systemd-resolved.service ]; then
  systemctl enable systemd-resolved
  ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf || true
else
  echo "systemd-resolved not present; skipping enable" >&2
fi

# SSH hardening: prohibit-password by default; firstboot will create user
passwd -l root
sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config
CHROOT

# overlays
sudo rsync -aHAX "$OVERLAYS_DIR"/ "$OUT"/

# ensure firstboot runs
sudo chroot "$OUT" systemctl enable puppy-firstboot.service

umount_binds
trap - EXIT
