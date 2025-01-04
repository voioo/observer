#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "Installing Observer..."

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Please run as root${NC}"
    exit 1
fi

# Create necessary directories
mkdir -p /etc/observer
mkdir -p /usr/local/bin

# Install binary
echo "Installing binary..."
cp observer /usr/local/bin/
chmod +x /usr/local/bin/observer

# Install config
echo "Installing configuration..."
if [ ! -f /etc/observer/config ]; then
    cp config.toml /etc/observer/config
else
    echo "Config file already exists, keeping existing configuration"
fi

# Install service
echo "Installing systemd service..."
cp observer.service /etc/systemd/system/

# Reload systemd and enable service
echo "Configuring service..."
systemctl daemon-reload
systemctl enable observer
systemctl start observer

echo -e "${GREEN}Installation complete!${NC}"
echo "Check service status with: systemctl status observer"
echo "View logs with: journalctl -u observer -f"