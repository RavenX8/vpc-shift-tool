# OpenVPC - Shift Tool

A free and open-source alternative to the VPC Shift Tool bundled with the VirPil control software package.

## Overview

OpenVPC Shift Tool is a utility designed for VirPil flight simulation controllers. It allows you to combine button inputs from multiple VirPil devices using logical operations (OR, AND, XOR), creating a "shift state" that can be sent to receiver devices. This enables more complex control schemes and button combinations for flight simulators.

## Features

- Connect to multiple VirPil devices simultaneously
- Configure source devices that provide button inputs
- Set up receiver devices that receive the combined shift state
- Choose between different logical operations (OR, AND, XOR) for each bit
- Automatic device detection for VirPil hardware
- Configuration saving and loading
- Cross-platform support (Windows and Linux)

## Installation

### Pre-built Binaries

Pre-built binaries for Windows and Linux are available in the GitHub Actions artifacts for each commit. You can find them by:

1. Going to the [Actions tab](https://github.com/RavenX8/vpc-shift-tool/actions)
2. Selecting the most recent successful workflow run
3. Downloading the appropriate artifact for your platform (Linux-x86_64_build or Windows-x86_64_build)

### Building from Source

#### Prerequisites

- Rust toolchain (install from [rustup.rs](https://rustup.rs/))
- For Linux: libudev-dev package

#### Build Steps

```bash
# Clone the repository
git clone https://github.com/RavenX8/vpc-shift-tool.git
cd vpc-shift-tool

# Build the release version
cargo build --release
```

The compiled binary will be available in `target/release/`.

### Linux Installation

On Linux, you need to install udev rules to access VirPil devices without root privileges:

```bash
# Using the Makefile
sudo make install

# Or manually
sudo cp udev/rules.d/70-vpc.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Usage

1. Launch the application
2. The main interface shows three sections:
   - **Sources**: Devices that provide button inputs
   - **Rules**: Logical operations to apply to each bit
   - **Receivers**: Devices that receive the combined shift state

3. Add source devices by selecting them from the dropdown menu
4. Configure which bits are active for each source
5. Set the logical operation (OR, AND, XOR) for each bit in the Rules section
6. Add receiver devices that will receive the combined shift state
7. Click "Start" to begin the shift operation

## Configuration

The application automatically saves your configuration to:
- Windows: `%APPDATA%\shift_tool.json`
- Linux: `~/.config/shift_tool.json`

## Troubleshooting

### Device Not Detected

- Ensure your VirPil devices are properly connected
- On Linux, verify udev rules are installed correctly
- Try refreshing the device list with the "Refresh Devices" button

### Permission Issues on Linux

If you encounter permission issues accessing the devices on Linux:

1. Ensure the udev rules are installed correctly
2. Log out and log back in, or reboot your system
3. Verify your user is in the "input" group: `groups $USER`

## License

GNU General Public License v3.0

## Author

RavenX8

## Links

- [GitHub Repository](https://github.com/RavenX8/vpc-shift-tool)
- [Gitea Repository](https://gitea.azgstudio.com/Raven/vpc-shift-tool)
