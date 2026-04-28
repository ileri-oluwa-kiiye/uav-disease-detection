BINARY = uav-flight-control
TARGET = thumbv7em-none-eabihf

.PHONY: all build flash clean

all: run

build:
	cargo build --release

run:
	cargo run --release

flash:
	cargo flash --chip STM32H743VITx --release

clean:
	cargo clean
