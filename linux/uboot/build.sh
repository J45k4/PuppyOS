git clone https://source.denx.de/u-boot/u-boot.git
cd u-boot
# Example: try your board’s defconfig (names vary by vendor/board)
# ROCK 5B often: rock5b-rk3588_defconfig
# Orange Pi 5/5B often: orangepi-5-rk3588_defconfig
make CROSS_COMPILE=aarch64-linux-gnu- <your_board>_defconfig
make -j$(nproc) CROSS_COMPILE=aarch64-linux-gnu-
# Outputs you need:
#   spl idbloader:  u-boot/idbloader.img
#   FIT image:      u-boot/u-boot.itb