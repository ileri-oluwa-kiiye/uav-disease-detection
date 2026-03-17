# uav-disease-detection
A drone for plant disease detection

## Flight Controller
The UAV flight controller runs on an ESP32-S3 N16R8. The flight control program is written in Rust using the esp_hal toolkit.

### Setup (Windows)
Step 1: Install the Rust compiler on your PC. See the [Rust website](https://rustup.rs/) for instructions.

Step 2: Run the following terminal commands to setup your windows PC development environment (one after the other):
`cargo install espup --locked`
`espup install`
`cargo install espflash --locked`

Step 3 (Optional): Install [Zed Editor](https://zed.dev/download) or [Visual Studio Code](https://code.visualstudio.com/) on your PC. If using Visual Studio Code, install the **Rust Analyzer** extension.
*This improves your developer experience when editing code and running commands from the IDE terminal.*

### Flashing (and Running) The Project
Step 1: Connect the ESP board to your PC via USB.
Step 2: Open the terminal (preferrably from the IDE) and run `cargo run --release`.
Step 3: Select the port (COM#) when prompted and select yes to always use the port.

The code is flashed onto the board and immediately executed.
