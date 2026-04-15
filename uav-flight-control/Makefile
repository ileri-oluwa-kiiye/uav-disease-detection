BINARY = uav-flight-control
TARGET = thumbv7em-none-eabihf
ELF = target/$(TARGET)/release/$(BINARY)
BIN = target/$(BINARY).bin

.PHONY: all build flash clean

all: build flash

build:
	cargo build --release
	arm-none-eabi-objcopy -O binary $(ELF) $(BIN)
	@echo "Done"

flash:
	-sudo dfu-util -a 0 -s 0x08000000:leave -D $(BIN)

clean:
	cargo clean
	rm -f $(BIN)
