# Installation Guide for OpenVPC Shift Tool

This guide provides detailed instructions for installing and setting up the OpenVPC Shift Tool on different operating systems.

## Windows Installation

### Using Pre-built Binary

1. Download the Windows build artifact (`Windows-x86_64_build`) from the [GitHub Actions](https://github.com/RavenX8/vpc-shift-tool/actions) page by selecting the most recent successful workflow run
2. The downloaded artifact is the executable binary itself (shift_tool.exe)
3. Place it in a location of your choice and run it

### Building from Source on Windows

1. Install the Rust toolchain from [rustup.rs](https://rustup.rs/)
2. Open Command Prompt or PowerShell
3. Clone the repository:
   ```
   git clone https://github.com/RavenX8/vpc-shift-tool.git
   cd vpc-shift-tool
   ```
4. Build the release version:
   ```
   cargo build --release
   ```
5. The executable will be available at `target\release\shift_tool.exe`

## Linux Installation

### Using Pre-built Binary

1. Download the Linux build artifact (`Linux-x86_64_build`) from the [GitHub Actions](https://github.com/RavenX8/vpc-shift-tool/actions) page by selecting the most recent successful workflow run
2. The downloaded artifact is the executable binary itself, no extraction needed. Just make it executable:
   ```
   chmod +x Linux-x86_64_build
   # Optionally rename it to something more convenient
   mv Linux-x86_64_build shift_tool
   ```
3. Create and install the udev rules for device access:
   ```
   sudo mkdir -p /etc/udev/rules.d/
   # Create the udev rule file
   echo -e '# Virpil Control devices\nSUBSYSTEM=="usb", ATTRS{idVendor}=="3344", TAG+="uaccess", GROUP:="input"' | sudo tee /etc/udev/rules.d/70-vpc.rules
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```
4. Make the binary executable and run it:
   ```
   chmod +x shift_tool
   ./shift_tool
   ```

### Building from Source on Linux

1. Install dependencies:
   ```
   # Ubuntu/Debian
   sudo apt install build-essential libudev-dev pkg-config

   # Fedora
   sudo dnf install gcc libudev-devel pkgconfig

   # Arch Linux
   sudo pacman -S base-devel
   ```

2. Install the Rust toolchain from [rustup.rs](https://rustup.rs/)

3. Clone the repository:
   ```
   git clone https://github.com/RavenX8/vpc-shift-tool.git
   cd vpc-shift-tool
   ```

4. Build and install using the Makefile:
   ```
   make
   sudo make install
   ```

   Or manually:
   ```
   cargo build --release
   sudo cp target/release/shift_tool /usr/local/bin/
   sudo mkdir -p /etc/udev/rules.d/
   sudo cp udev/rules.d/70-vpc.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

## Verifying Installation

After installation, you can verify that the application can detect your VirPil devices:

1. Connect your VirPil device(s) to your computer
2. Launch the OpenVPC Shift Tool
3. Click the "Refresh Devices" button
4. Your devices should appear in the dropdown menus

If devices are not detected:

- On Windows, ensure you have the correct drivers installed
- On Linux, verify the udev rules are installed correctly and you've reloaded the rules
- Check that your devices are properly connected and powered on

## Configuration Location

The application stores its configuration in:

- Windows: `%APPDATA%\shift_tool.json`
- Linux: `~/.config/shift_tool.json`

## Uninstallation

### Windows

Simply delete the application files.

### Linux

If installed using the Makefile:
```
sudo rm /usr/local/bin/shift_tool
sudo rm /etc/udev/rules.d/70-vpc.rules
```

## Troubleshooting

### Linux Permission Issues

If you encounter permission issues accessing the devices on Linux:

1. Ensure the udev rules are installed correctly
2. Add your user to the input group:
   ```
   sudo usermod -a -G input $USER
   ```
3. Log out and log back in, or reboot your system
4. Verify your user is in the input group:
   ```
   groups $USER
   ```

### Windows Device Access Issues

If the application cannot access devices on Windows:

1. Ensure no other application (like the official VPC software) is currently using the devices
2. Try running the application as Administrator (right-click, "Run as Administrator")
3. Check Device Manager to ensure the devices are properly recognized by Windows
