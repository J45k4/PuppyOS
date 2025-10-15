# PuppyOS Linux Build System

This directory contains the minimal, headless PuppyOS image builder. It assembles an Ubuntu 24.04 (Noble) ARM64 root filesystem, pairs it with board-specific kernel and U-Boot prebuilts, and generates bootable disk images.

## Prerequisites

Ensure the following packages are installed on the build host (Ubuntu/Debian names shown):

```
sudo apt install \
  debootstrap qemu-user-static binfmt-support yq \
  parted kpartx rsync dosfstools e2fsprogs \
  rkdeveloptool cloud-guest-utils
```

You also need prebaked firmware assets for your board:

- Place U-Boot binaries under `uboot/prebuilts/<board>/` (`idbloader.img`, `u-boot.itb`).
- Place the kernel image, DTBs, and optional initrd under `kernels/prebuilts/<board>/` (`Image`, `*.dtb`, `initrd.img`).

The default board configuration is `opi5b` (Orange Pi 5B). You can add new boards by creating a YAML file in `configs/boards/` and dropping the matching prebuilts.

## Building an Image

From the repository root (or the `linux/` directory), run:

```
make -C linux all BOARD=opi5b
```

This performs the following steps:

1. `make rootfs` – Runs `build/scripts/build-rootfs.sh` to create the Ubuntu Noble root filesystem in `linux/work/rootfs/`.
2. `make image` – Calls `build/scripts/assemble-image.sh` to lay out the GPT disk image, copy the rootfs, and install kernel/U-Boot assets.

The resulting image is written to `linux/images/puppyos-noble-min-<board>.img`.

### Rebuilding Components

- `make -C linux rootfs` rebuilds only the root filesystem.
- `make -C linux image` repacks an image from an existing rootfs.
- `make -C linux clean` removes the `work/` directory and any generated images.

## Flashing to a Device

To flash an Orange Pi 5B (or other Rockchip-based boards) over USB OTG using Rockchip's tooling:

```
make -C linux flash BOARD=opi5b
```

This invokes `build/scripts/flash-rk.sh`, which wraps `rkdeveloptool`.

## First Boot Behavior

On first boot, the `puppy-firstboot` service runs to:

- Expand the root filesystem to fill the target device (uses `growpart` + `resize2fs`).
- Create a `puppy` user with passwordless sudo.
- Enable password SSH logins only if `PUPPY_PW` is set during image build.

The root account remains locked, and SSH password logins are disabled by default. To embed SSH keys, add them via overlays (e.g., `overlays/home/puppy/.ssh/authorized_keys`).
