# Contributing to OpenVPC Shift Tool

Thank you for your interest in contributing to the OpenVPC Shift Tool! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Please be respectful and considerate of others when contributing to this project. We aim to foster an inclusive and welcoming community.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```
   git clone https://github.com/YOUR-USERNAME/vpc-shift-tool.git
   cd vpc-shift-tool
   ```
3. Add the original repository as an upstream remote:
   ```
   git remote add upstream https://github.com/RavenX8/vpc-shift-tool.git
   ```
4. Create a new branch for your changes:
   ```
   git checkout -b feature/your-feature-name
   ```

## Development Environment Setup

1. Install the Rust toolchain from [rustup.rs](https://rustup.rs/)
2. Install dependencies:
   - Windows: No additional dependencies required
   - Linux: `sudo apt install libudev-dev pkg-config` (Ubuntu/Debian)

3. Build the project:
   ```
   cargo build
   ```

4. Run the application:
   ```
   cargo run
   ```

## Making Changes

1. Make your changes to the codebase
2. Write or update tests as necessary
3. Ensure all tests pass:
   ```
   cargo test
   ```
4. Format your code:
   ```
   cargo fmt
   ```
5. Run the linter:
   ```
   cargo clippy
   ```

## Commit Guidelines

- Use clear and descriptive commit messages
- Reference issue numbers in your commit messages when applicable
- Keep commits focused on a single change
- Use the present tense ("Add feature" not "Added feature")

## Pull Request Process

1. Update your fork with the latest changes from upstream:
   ```
   git fetch upstream
   git rebase upstream/main
   ```
2. Push your changes to your fork:
   ```
   git push origin feature/your-feature-name
   ```
3. Create a pull request through the GitHub interface
4. Ensure your PR description clearly describes the changes and their purpose
5. Link any related issues in the PR description

## Code Style

- Follow the Rust style guidelines
- Use meaningful variable and function names
- Add comments for complex logic
- Document public functions and types

## Project Structure

- `src/main.rs`: Application entry point and main structure
- `src/about.rs`: About screen information
- `src/config.rs`: Configuration handling
- `src/device.rs`: Device representation and management
- `src/hid_worker.rs`: HID communication worker thread
- `src/state.rs`: Application state management
- `src/ui.rs`: User interface components
- `src/util.rs`: Utility functions and constants

## Adding Support for New Devices

If you're adding support for new device types:

1. Update the device detection logic in `device.rs`
2. Add any necessary report format definitions in `util.rs`
3. Test with the actual hardware if possible
4. Document the new device support in your PR

## Testing

- Write unit tests for new functionality
- Test on both Windows and Linux if possible
- Test with actual VirPil hardware if available

## Documentation

- Update the README.md with any new features or changes
- Document new functions and types with rustdoc comments
- Update TECHNICAL.md for significant architectural changes

## Reporting Issues

If you find a bug or have a suggestion for improvement:

1. Check if the issue already exists in the [GitHub Issues](https://github.com/RavenX8/vpc-shift-tool/issues)
2. If not, create a new issue with:
   - A clear title and description
   - Steps to reproduce the issue
   - Expected and actual behavior
   - System information (OS, Rust version, etc.)
   - Screenshots if applicable

## License

By contributing to this project, you agree that your contributions will be licensed under the project's [GNU General Public License v3.0](LICENSE).
