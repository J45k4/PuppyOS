```
git clone https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git
cd linux
# Option A: start from defconfig, then add drivers
make ARCH=arm64 defconfig
# Enable / check these (menuconfig):
#   Device tree + Rockchip DRM:
#     CONFIG_DRM=y
#     CONFIG_DRM_ROCKCHIP=y
#     CONFIG_DRM_DISPLAY_CONNECTOR=y
#   GPU:
#     CONFIG_DRM_PANTHOR=y          # Valhall GPUs incl. Mali-G610
#     CONFIG_DRM_PANFROST=y         # (kept on; harmless fallback)
#   HDMI/bridges common helpers are usually on in defconfig
#   Audio (optional): CONFIG_SND_SOC_ROCKCHIP, i2s codecs as needed
#   Filesystems: ext4, vfat
#   Networking: e1000e/r8169/USB NICs as needed, 80211/cfg80211 if Wi-Fi
#   Serial console:
#     CONFIG_SERIAL_8250=y
#     CONFIG_SERIAL_8250_CONSOLE=y
#   Rockchip SoC bits:
#     CONFIG_ROCKCHIP_PM_DOMAINS=y
#     CONFIG_ROCKCHIP_IODOMAIN=y
#     CONFIG_PINCTRL_ROCKCHIP=y
#     CONFIG_ROCKCHIP_THERMAL=y
#   V4L2 decode (optional/experimental):
#     CONFIG_MEDIA_SUPPORT=y
#     CONFIG_V4L_MEM2MEM_DRIVERS=y
#     CONFIG_VIDEO_HANTRO=y (generic)
#     CONFIG_VIDEO_ROCKCHIP_VDEC=y (if offered on your tree)
make ARCH=arm64 menuconfig
make -j$(nproc) ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image dtbs modules
```