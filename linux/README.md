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

- `make BOARD=opi5b` (or `make all`) – builds the kernel image/DTBs, packages U-Boot, creates the rootfs, and assembles `build/images/puppyos-<board>.img`.
- `make kernel` – runs `scripts/build-kernel.sh`, depositing `build/kernel/<board>/`.
- `make uboot` – runs `scripts/build-uboot.sh`, depositing `build/uboot/<board>/`.
- `make rootfs` – runs `scripts/build-rootfs.sh` with `overlays/` and `configs/distro/ubuntu-noble-min.yaml`, storing the rootfs under `build/rootfs/`.
- `make image` – bundles the above artifacts using `scripts/assemble-image.sh`.
- `make flash` – pushes the assembled image over USB with `scripts/flash-rk.sh`.
- `make clean` – nukes `build/`.
- Skip knobs: set `SKIP_KERNEL=1`, `SKIP_UBOOT=1`, or `SKIP_ROOTFS=1` with `make image` if you already have artifacts staged under `build/` and only want to repack the image.
- Overlays: `OVERLAYS` defaults to `overlays/`. If `linux/local-overlays/` exists (git-ignored), it is applied automatically after `OVERLAYS` so you can inject private files such as SSH keys without passing extra args.
- Serial console: set `BAUDRATE=<bps>` (default `115200`) on `make` and it will apply to both U-Boot (`CONFIG_BAUDRATE`) and the kernel bootargs; to open the board console from your workstation run `sudo picocom -b 115200 /dev/ttyUSB0` (or adjust the baud and device name to match your adapter) once the system starts booting.
- Early console: `EARLYCON=1` (default) injects `earlycon=uart8250,mmio32,<addr>` into bootargs; override the address with `EARLYCON_ADDR` (default `0xfeb50000`) or disable entirely with `EARLYCON=0`.

Pass `BOARD`, `BUILD_ROOT`, or `FRAGMENT` if you want to target other RK3588 configurations or custom fragments.
Pass `KCONFIG=<path>` to `make kernel` if you need to override the merged fragment and supply your own `.config`; `scripts/build-kernel.sh` makes the path absolute *before* entering the kernel tree so a `KCONFIG=./config-6.1.43-rockchip-rk3588` from the repo root “just works” and the file ends up copied into `linux/kernel/.config` before building.

## Kernel build notes

The kernel tree lives in `kernel/`. Keep it up to date yourself (e.g., `git pull` inside `kernel/`) or replace it with your preferred source. To configure/build manually:

```bash
cd linux/kernel
make ARCH=arm64 defconfig
./scripts/kconfig/merge_config.sh -m .config linux/kernels/configs/rk3588_mainline_min.hdmi.fragment
make ARCH=arm64 olddefconfig
make -j$(nproc) ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image dtbs modules
sudo make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- INSTALL_MOD_PATH=/path/to/work/rootfs modules_install
```

`scripts/mkconfig-mainline.sh` now defaults to `rk3588_mainline_min.hdmi.fragment` so the build enables the Rockchip HDMI/VOP stack; pass `FRAGMENT=linux/kernels/configs/rk3588_mainline_min.headless.fragment` if you need the smaller headless set. The helper script keeps the merge interactive-free, and `scripts/build-kernel.sh` clones https://git.kernel.org/.../stable/linux.git into `linux/kernel/` if it is missing, keeps it synced, runs the helper, and copies `Image` plus RK3588 DTBs into `build/kernel/<board>/`.

### Fragment overrides

`scripts/mkconfig-mainline.sh` merges the requested fragment on top of `make ARCH=arm64 defconfig` and immediately runs `make olddefconfig` so all other symbols stay at their defaults. Only the `CONFIG_*` lines in the fragment actually move away from the defconfig, so the existing HDMI/headless fragments in `linux/kernels/configs/` contain just the overrides the repo cares about. To force additional flags, drop a new fragment (for example `linux/kernels/configs/my-special.fragment`) with the minimal `CONFIG_FOO=y`/`CONFIG_BAR=m` lines you need and build with `FRAGMENT=linux/kernels/configs/my-special.fragment make image`; the merge will respect your overrides while leaving everything else untouched.

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
Pass `BAUDRATE=<bps>` (defaults to `115200`) to `make` or `make uboot` if you want `CONFIG_BAUDRATE` (and the matching kernel bootarg) to use a different serial speed.

## Overlays and First Boot

`scripts/build-rootfs.sh` copies the contents of `overlays/` into the rootfs after debootstrap, so drop SSH keys, systemd units, or user config there. The provided distro config disables password SSH logins, enables `puppy-firstboot.service`, and hardens the system (see `configs/distro/ubuntu-noble-min.yaml`). If `linux/local-overlays/` exists (git-ignored), it is layered automatically after `overlays/`—ideal for private `authorized_keys` or other secrets without touching the repo. Example:

```
mkdir -p linux/local-overlays/home/puppy/.ssh
cp ~/.ssh/id_rsa.pub linux/local-overlays/home/puppy/.ssh/authorized_keys
make -C linux BOARD=opi5b
```

## Flashing and Writing SD Cards

- `scripts/make-opi-sd.sh /dev/sdX` – writes the SD card directly using the kernel, DTB, rootfs, and U-Boot artifacts produced under `build/`. It expects Rockchip `idbloader.img` + `u-boot.itb` when `SOC=rockchip`.
- `make flash` – wraps `rkdeveloptool` for USB flashing (`scripts/flash-rk.sh`).

- `scripts/write-image.sh <image> <device>` – safety wrapper around `dd` that validates the image (size plus partition table), confirms the target is removable/not the host disk, runs a read-back comparison after `dd`, prompts before writing, and ejects the device when possible so you can flash `build/images/puppyos-<board>.img` confidently.

Ensure the target device is unmounted before running the SD card script. All boot assets are pulled from `build/`; rerun `make` if you change kernel/U-Boot/rootfs inputs.
