# PuppyOS Linux Build System

This tree hosts the build system for the headless PuppyOS Linux image. `linux/` now holds:

- `kernel/` – the Linux source tree. Keep it updated via `git pull` in that directory (or replace it with your preferred kernel tree).
- `uboot/` – the Rockchip U-Boot source tree with helper scripts to package `idbloader.img` / `u-boot.itb`.
- `rkbin/` – Rockchip blobs that U-Boot needs (`RKBOOT`, `RKTRUST`, TPL/BL31 binaries, etc.).
- `configs/`, `overlays/`, `scripts/` – build inputs and helper scripts.
- `build/` – (ignored) this is where the kernels, U-Boot artifacts, rootfs cache, and final images land.

## Dependencies

Install the tooling before building:

```
sudo apt install \
  debootstrap qemu-user-static binfmt-support yq \
  flex bison \
  parted kpartx rsync dosfstools e2fsprogs \
  rkdeveloptool cloud-guest-utils
```

You can automate the above by running `./scripts/install_dependencies.sh`; it also reminds you to install a cross-toolchain and place Rockchip blobs into `linux/rkbin/`.

You also need to keep the kernel, U-Boot, and Rockchip blobs repositories in `kernel/`, `uboot/`, and `rkbin/`.

## Build Workflow

The single `Makefile` in this directory now orchestrates the kernel, U-Boot, and rootfs builds:

- `make BOARD=opi5b` (or `make all`) – builds the kernel image/DTBs, packages U-Boot, creates the rootfs, and assembles `build/images/puppyos-noble-min-<board>.img`.
- `make kernel` – runs `scripts/build-kernel.sh`, depositing `build/kernel/<board>/`.
- `make uboot` – runs `scripts/build-uboot.sh`, depositing `build/uboot/<board>/`.
- `make rootfs` – runs `scripts/build-rootfs.sh` with `overlays/` and `configs/distro/ubuntu-noble-min.yaml`, storing the rootfs under `build/rootfs/`.
- `make image` – bundles the above artifacts using `scripts/assemble-image.sh`.
- `make flash` – pushes the assembled image over USB with `scripts/flash-rk.sh`.
- `make clean` – nukes `build/`.

Pass `BOARD`, `BUILD_ROOT`, or `FRAGMENT` if you want to target other RK3588 configurations or custom fragments.

## Kernel build notes

The kernel tree lives in `kernel/`. Keep it up to date yourself (e.g., `git pull` inside `kernel/`) or replace it with your preferred source. To configure/build manually:

```bash
cd linux/kernel
make ARCH=arm64 defconfig
./scripts/kconfig/merge_config.sh -m .config linux/kernels/configs/rk3588_mainline_min.headless.fragment
make ARCH=arm64 olddefconfig
make -j$(nproc) ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image dtbs modules
sudo make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- INSTALL_MOD_PATH=/path/to/work/rootfs modules_install
```

`scripts/mkconfig-mainline.sh` encapsulates the first three steps if you want to reuse the fragment elsewhere. It now accepts defaults automatically when `olddefconfig` asks about new Kconfig options, so you can run it non-interactively. `scripts/build-kernel.sh` will clone https://git.kernel.org/.../stable/linux.git into `linux/kernel/` if it is missing, keep it synced, run the helper, and copy `Image` plus RK3588 DTBs into `build/kernel/<board>/`.

## U-Boot notes

U-Boot is fetched under `uboot/`. Install the native dependencies before you build:

```
sudo apt-get install swig libgnutls28-dev python3-pyelftools
```

You also need the Rockchip binaries under `rkbin/` so `build-uboot.sh` can provide `ROCKCHIP_TPL` and `BL31`. After cloning `rkbin`, export the required blobs (or rely on `build-uboot.sh` defaults):

```bash
export ROCKCHIP_TPL=/path/to/rkbin/bin/rk35/rk3588_ddr_lp4_2112MHz_lp5_2400MHz_v1.19.bin
export BL31=/path/to/rkbin/bin/rk35/rk3588_bl31_v1.51.elf
```

`build-uboot.sh` keeps `u-boot`/`rkbin` in sync, runs `orangepi-5-plus-rk3588_defconfig`, builds, and copies `idbloader.img`/`u-boot.itb` into `build/uboot/<board>/`. Provide `TEE` if you also need an OP-TEE image.

## Overlays and First Boot

`scripts/build-rootfs.sh` copies the contents of `overlays/` into the rootfs after debootstrap, so drop SSH keys, systemd units, or user config there. The provided distro config disables password SSH logins, enables `puppy-firstboot.service`, and hardens the system (see `configs/distro/ubuntu-noble-min.yaml`).

## Flashing and Writing SD Cards

- `scripts/make-opi-sd.sh /dev/sdX` – writes the SD card directly using the kernel, DTB, rootfs, and U-Boot artifacts produced under `build/`. It expects Rockchip `idbloader.img` + `u-boot.itb` when `SOC=rockchip`.
- `make flash` – wraps `rkdeveloptool` for USB flashing (`scripts/flash-rk.sh`).

Ensure the target device is unmounted before running the SD card script. All boot assets are pulled from `build/`; rerun `make` if you change kernel/U-Boot/rootfs inputs.
