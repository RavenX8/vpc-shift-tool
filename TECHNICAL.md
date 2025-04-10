# OpenVPC Shift Tool - Technical Documentation

This document provides technical details about the OpenVPC Shift Tool application, its architecture, and how it works internally.

## Architecture Overview

The application is built using Rust with the following main components:

1. **GUI Layer**: Uses the `eframe` crate (egui) for cross-platform GUI
2. **HID Communication**: Uses the `hidapi` crate to communicate with VirPil devices
3. **Configuration Management**: Uses the `fast_config` crate for saving/loading settings
4. **Worker Thread**: Background thread that handles device communication

## Core Components

### Main Application Structure

The main application is represented by the `ShiftTool` struct in `src/main.rs`, which contains:

- State management
- Device list
- Shared state between UI and worker thread
- Configuration data

### Modules

- **about.rs**: Contains application information and about screen text
- **config.rs**: Configuration data structures and serialization
- **device.rs**: Device representation and management
- **hid_worker.rs**: Background worker thread for HID communication
- **state.rs**: Application state enum
- **ui.rs**: User interface drawing and event handling
- **util.rs**: Utility functions and constants

## Data Flow

1. The application scans for VirPil devices (vendor ID 0x3344)
2. User selects source and receiver devices in the UI
3. When "Start" is clicked, a worker thread is spawned
4. The worker thread:
   - Opens connections to all configured devices
   - Reads input from source devices
   - Applies logical operations based on configuration
   - Writes the resulting shift state to receiver devices
5. Shared state (protected by mutexes) is used to communicate between the UI and worker thread

## Device Communication

### Device Detection

Devices are detected using the HID API, filtering for VirPil's vendor ID (0x3344). The application creates `VpcDevice` objects for each detected device, which include:

- Vendor ID and Product ID
- Device name and firmware version
- Serial number
- Usage page/ID

### HID Protocol

The application supports different report formats based on device firmware versions. The worker thread:

1. Reads HID reports from source devices
2. Extracts button states from the reports
3. Applies logical operations (OR, AND, XOR) to combine states
4. Formats the combined state into HID reports
5. Sends the reports to receiver devices

## Configuration

Configuration is stored in JSON format using the `fast_config` crate. The configuration includes:

- Source devices (vendor ID, product ID, serial number, enabled bits)
- Receiver devices (vendor ID, product ID, serial number, enabled bits)
- Shift modifiers (logical operations for each bit)

## Threading Model

The application uses a main UI thread and a separate worker thread:

1. **Main Thread**: Handles UI rendering and user input
2. **Worker Thread**: Performs HID communication in the background

Thread synchronization is achieved using:
- `Arc<Mutex<T>>` for shared state
- `Arc<(Mutex<bool>, Condvar)>` for signaling thread termination

## Linux-Specific Features

On Linux, the application requires udev rules to access HID devices without root privileges. The rule is installed to `/etc/udev/rules.d/70-vpc.rules` and contains:

```
# Virpil Control devices
SUBSYSTEM=="usb", ATTRS{idVendor}=="3344", TAG+="uaccess", GROUP:="input"
```

## Building and Deployment

The application can be built using Cargo:

```bash
cargo build --release
```

A Makefile is provided for easier installation on Linux, which:
1. Builds the application
2. Installs the binary to `/usr/local/bin`
3. Installs udev rules to `/etc/udev/rules.d/`

## Future Development

Potential areas for enhancement:
- Support for additional device types
- More complex logical operations
- Custom button mapping
- Profile management
- Integration with game APIs
