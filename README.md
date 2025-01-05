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

### Arch Linux (AUR)
```bash
yay -S observer
```

### Manual Installation
1. Download the appropriate archive for your architecture from the [releases page](https://github.com/voioo/observer/releases)
2. Extract and install:
```bash
tar xzf observer-linux-*.tar.gz
sudo ./install.sh
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
sudo ./install.sh
```

## Architecture Support

- [x] x86_64 (AMD64)
- [ ] aarch64 (ARM64)
- [ ] armv7 (32-bit ARM)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the [0BSD License](LICENSE).

## Acknowledgments

- Inspired by various power management tools and CPU governors
- Thanks to the Rust community for excellent crates