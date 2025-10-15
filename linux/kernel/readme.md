## Mainline RK3588 (headless) quick-start

```bash
git clone https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git
cd linux
```

1. Use the ready fragment at `linux/kernels/configs/rk3588_mainline_min.headless.fragment` (covers boot, storage, Ethernet, NVMe/USB, serial console, DRM incl. Panthor/Panfrost; media/ISP/NPU are disabled by default).
2. Merge it the kernel-native way:

   ```bash
   make ARCH=arm64 defconfig
   ./scripts/kconfig/merge_config.sh -m .config \
     /path/to/puppyos/linux/kernels/configs/rk3588_mainline_min.headless.fragment
   make ARCH=arm64 olddefconfig
   ```

   Tip: `ttyS2,1500000n8` is the usual serial console; adjust later if your board differs.

3. Build artifacts:

   ```bash
   make -j$(nproc) ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image dtbs modules
   ```

4. Install modules into your rootfs (tweak the path to your staging rootfs):

   ```bash
   sudo make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- \
     INSTALL_MOD_PATH=/path/to/puppyos/linux/work/rootfs modules_install
   ```

5. Copy prebuilts so the image builder can pick them up:

   ```bash
   cp arch/arm64/boot/Image /path/to/puppyos/linux/kernels/prebuilts/<board>/
   cp arch/arm64/boot/dts/rockchip/rk3588-*.dtb /path/to/puppyos/linux/kernels/prebuilts/<board>/
   ```

6. Optional helper (automation): use `build/scripts/mkconfig-mainline.sh` (contents below) to run:

   ```bash
   #!/usr/bin/env bash
   set -euo pipefail
   KDIR="${1:-$PWD}"
   FRAG="${2:-kernels/configs/rk3588_mainline_min.headless.fragment}"

   cd "$KDIR"
   make ARCH=arm64 defconfig
   ./scripts/kconfig/merge_config.sh -m .config "$FRAG"
   make ARCH=arm64 olddefconfig
   echo "Merged config written to $KDIR/.config"
   ```

   Usage: `build/scripts/mkconfig-mainline.sh /path/to/linux`
   Pass the fragment path as a second argument if it lives outside the kernel tree (for example `build/scripts/mkconfig-mainline.sh /path/to/linux /path/to/puppyos/linux/kernels/configs/rk3588_mainline_min.headless.fragment`).

7. U-Boot note for Orange Pi 5/5B-class boards: try `orangepi-5-rk3588_defconfig`. Drop the resulting `idbloader.img` and `u-boot.itb` into `uboot/prebuilts/<board>/`.
