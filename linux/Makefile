BOARD ?= opi5b
BUILD_ROOT ?= $(CURDIR)/build
IMG ?= puppyos-noble-min-$(BOARD).img

ROOTFS_DIR := $(BUILD_ROOT)/rootfs
KERNEL_DEST := $(BUILD_ROOT)/kernel/$(BOARD)
UBOOT_DEST := $(BUILD_ROOT)/uboot/$(BOARD)
IMAGE_FILE := $(BUILD_ROOT)/images/$(IMG)
SCRIPTS := $(CURDIR)/scripts
SCRIPT_FILES := $(wildcard $(SCRIPTS)/*.sh)

all: image

.PHONY: script_perms
script_perms:
	chmod +x $(SCRIPT_FILES)

kernel: script_perms
	mkdir -p $(BUILD_ROOT)/kernel
	BUILD_ROOT="$(BUILD_ROOT)/kernel" $(SCRIPTS)/build-kernel.sh $(BOARD)

uboot: script_perms
	mkdir -p $(BUILD_ROOT)/uboot
	BUILD_ROOT="$(BUILD_ROOT)/uboot" $(SCRIPTS)/build-uboot.sh $(BOARD)

rootfs: script_perms
	mkdir -p $(BUILD_ROOT)
	$(SCRIPTS)/build-rootfs.sh overlays configs/distro/ubuntu-noble-min.yaml $(ROOTFS_DIR)

image: script_perms kernel uboot rootfs
	mkdir -p $(dir $(IMAGE_FILE))
	$(SCRIPTS)/assemble-image.sh \
		--board $(BOARD) \
		--rootfs $(ROOTFS_DIR) \
		--uboot $(UBOOT_DEST) \
		--kernel $(KERNEL_DEST) \
		--output $(IMAGE_FILE)

flash: image
	$(SCRIPTS)/flash-rk.sh $(IMAGE_FILE)

clean:
	rm -rf $(BUILD_ROOT)
