BOARD ?= opi5b
BUILD_ROOT ?= $(CURDIR)/build
IMG ?= puppyos-$(BOARD).img
SKIP_KERNEL ?= 0
SKIP_UBOOT ?= 0
SKIP_ROOTFS ?= 0

ROOTFS_DIR := $(BUILD_ROOT)/rootfs
KERNEL_DEST := $(BUILD_ROOT)/kernel/$(BOARD)
UBOOT_DEST := $(BUILD_ROOT)/uboot/$(BOARD)
IMAGE_FILE := $(BUILD_ROOT)/images/$(IMG)
SCRIPTS := $(CURDIR)/scripts
SCRIPT_FILES := $(wildcard $(SCRIPTS)/*.sh)
OVERLAYS ?= overlays
LOCAL_OVERLAYS ?=
ifneq ("$(wildcard $(CURDIR)/local-overlays)","")
LOCAL_OVERLAYS := $(CURDIR)/local-overlays
endif

all: image

IMAGE_DEPS := script_perms
ifeq ($(SKIP_KERNEL),0)
IMAGE_DEPS += kernel
endif
ifeq ($(SKIP_UBOOT),0)
IMAGE_DEPS += uboot
endif
ifeq ($(SKIP_ROOTFS),0)
IMAGE_DEPS += rootfs
endif

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
	$(SCRIPTS)/build-rootfs.sh $(OVERLAYS) configs/distro/ubuntu-noble-min.yaml $(ROOTFS_DIR) $(LOCAL_OVERLAYS)

image: $(IMAGE_DEPS)
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
