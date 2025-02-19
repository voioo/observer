# Observer

A dynamic CPU core manager for Linux systems that "intelligently" manages CPU cores based on system load and power state to help reduce power consumption.

## Features

- Dynamic core management:
  - Automatically reduces active cores when running on battery
  - Scales cores up/down based on real-time CPU load
  - Proper handling of CPU HyperThreading pairs
  - Smooth transitions between states
  - Always maintains system responsiveness with minimum core count

- Configurable settings:
  - Battery mode core percentage
  - CPU load thresholds for scaling
  - Minimum core count
  - Check intervals
  - Core transition delays

- System integration:
  - Runs as a systemd service
  - Graceful shutdown handling
  - Proper logging with different verbosity levels
  - Configuration file in TOML format

## Installation

### Quick Install

```bash
curl -sL "https://github.com/voioo/observer/releases/latest/download/observer-linux-amd64.tar.gz" | sudo bash -c 'tar xz -C /tmp && bash /tmp/install.sh'
```

#### Uninstall

```bash
curl -sL "https://raw.githubusercontent.com/voioo/observer/main/uninstall.sh" | sudo bash
```

### Arch Linux (AUR)
```bash
yay -S observer
```

### Manual Installation
1. Download the appropriate archive for your architecture from the [releases page](https://github.com/voioo/observer/releases)
2. Extract and install:
```bash
tar xzf observer-linux-*.tar.gz
sudo chmod +x install.sh && sudo ./install.sh
```

## Configuration

The configuration file is located at `/etc/observer/config.toml`:

```toml
# Percentage of cores to enable when on battery (1-100)
battery_core_percentage = 50

# Delay in milliseconds between enabling/disabling each core
transition_delay_ms = 500

# How often to check power state and CPU load (in seconds)
check_interval_sec = 5

# CPU load threshold percentage to trigger core count adjustment
cpu_load_threshold = 40.0

# Minimum number of cores to keep enabled
min_cores = 2
```

## Usage

Observer runs as a systemd service. Control it using:

```bash
# Start the service
sudo systemctl start observer

# Enable on boot
sudo systemctl enable observer

# Check status
sudo systemctl status observer

# View logs
sudo journalctl -u observer -f
```

## Building from Source

### Prerequisites
- Rust toolchain (1.70.0 or newer)
- Cargo

### Build Steps
```bash
# Clone the repository
git clone https://github.com/voioo/observer.git
cd observer

# Build
cargo build --release

# Install (optional)
sudo chmod +x install.sh && sudo ./install.sh
```

## Architecture Support

- [x] x86_64 (AMD64) - Full support
- [x] aarch64 (ARM64) - Full support
- [x] armv7 (32-bit ARM) - Basic support
  - Note: Some ARM systems might have limited CPU core control capabilities
  - Performance might vary based on specific hardware

### Architecture-Specific Considerations
- **x86_64**: Full support for all features including HyperThreading detection
- **aarch64**: Full support, optimized for modern ARM64 processors
- **armv7**: Basic support, some features might be limited by hardware capabilities

## Contributing

Contributions are welcome! Check the [contributing guide](https://github.com/voioo/observer/blob/main/CONTRIBUTING.md) and feel free to submit a PR.

## License

This project is licensed under the [0BSD License](LICENSE).

## Acknowledgments

- Inspired by various power management tools and CPU governors
- Thanks to the Rust community for excellent crates

### Installation

Download the appropriate archive for your architecture:
- AMD64 (x86_64): `observer-linux-amd64.tar.gz`
- ARM64 (aarch64): `observer-linux-arm64.tar.gz`
- ARMv7 (32-bit): `observer-linux-armv7.tar.gz`
