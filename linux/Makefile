BOARD ?= opi5b
IMG ?= puppyos-noble-min-$(BOARD)

all: image

rootfs:
sudo ./build/scripts/build-rootfs.sh overlays configs/distro/ubuntu-noble-min.yaml work/rootfs

image: rootfs
sudo ./build/scripts/assemble-image.sh \
  --board $(BOARD) \
  --rootfs work/rootfs \
  --uboot uboot/prebuilts/$(BOARD) \
  --kernel kernels/prebuilts/$(BOARD) \
  --output images/$(IMG).img

flash:
sudo ./build/scripts/flash-rk.sh images/$(IMG).img

clean:
sudo rm -rf work images/*.img images/*.xz
