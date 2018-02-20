TARGET ?= raspi2
VERSION ?= release
KERNEL = rustberry-kernel
SRC_DIR = src
BUILD_DIR = target/$(TARGET)/$(VERSION)

ASSEMBLY_OBJECTS = $(BUILD_DIR)/boot.o
KERNEL_RUST_LIB = $(BUILD_DIR)/librustberry.a
LINKER_SCRIPT = src/linker.ld

ifeq ($(TARGET), raspi2)
	AS_FLAGS = --defsym CORTEX_A7=1
else
	AS_FLAGS =
endif

ifeq ($(VERSION), release)
	XARGO_FLAGS = --release --features "$(TARGET)"
else
	XARGO_FLAGS = --features "$(TARGET)"
endif

all: $(BUILD_DIR)/$(KERNEL).img

run: $(BUILD_DIR)/$(KERNEL).elf
	qemu-system-arm -m 256 -M raspi2 -serial stdio -display none -kernel $<

clean:
	rm -rf target

$(BUILD_DIR)/$(KERNEL).img: $(BUILD_DIR)/$(KERNEL).elf
	arm-none-eabi-objcopy $< -O binary $@

$(BUILD_DIR)/$(KERNEL).elf: xargo $(ASSEMBLY_OBJECTS)
	arm-none-eabi-ld --gc-sections -T $(LINKER_SCRIPT) -o $@ \
		$(ASSEMBLY_OBJECTS) $(KERNEL_RUST_LIB)

$(BUILD_DIR)/%.o: $(SRC_DIR)/%.s
	arm-none-eabi-as $(AS_FLAGS) $< -o $@

xargo:
	RUST_TARGET_PATH=$(shell pwd) xargo build --target $(TARGET) \
		$(XARGO_FLAGS)

.PHONY: all clean run xargo
