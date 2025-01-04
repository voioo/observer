# Observer

Dynamically manages CPU cores based on power state and system load.

## Features

- Automatically reduces active cores when running on battery
- Dynamically scales cores based on CPU load
- Proper handling of HyperThreading pairs
- Configurable thresholds and settings
- Systemd service integration

## Quick Install

Install the latest version with:

```bash
curl -sSL https://raw.githubusercontent.com/voioo/observer/main/install.sh | sudo bash
```

Or download and verify a specific release:

```bash
# Download latest release
VERSION=$(curl -s https://api.github.com/repos/voioo/observer/releases/latest | grep -oP '"tag_name": "\K(.*)(?=")')
wget https://github.com/voioo/observer/releases/download/$VERSION/observer-linux-amd64.tar.gz
wget https://github.com/voioo/observer/releases/download/$VERSION/observer-linux-amd64.sha256

# Verify checksum
sha256sum -c observer-linux-amd64.sha256

# Extract and install
tar xzf observer-linux-amd64.tar.gz
sudo ./install.sh
```

## Configuration

Edit `/etc/observer/config`:

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

The service starts automatically after installation. Control it with:

```bash
# Check status
sudo systemctl status observer

# View logs
sudo journalctl -u observer -f

# Restart service
sudo systemctl restart observer

# Stop service
sudo systemctl stop observer
```

## Building from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/voioo/observer.git
cd observer
cargo build --release

# Install
sudo ./install.sh
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.