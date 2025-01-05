#!/usr/bin/env bash

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BINARY_PATH="/usr/local/bin/observer"
CONFIG_DIR="/etc/observer"
CONFIG_FILE="$CONFIG_DIR/config.toml"
SERVICE_FILE="/etc/systemd/system/observer.service"

info() { echo -e "${BLUE}INFO:${NC} $1"; }
warn() { echo -e "${YELLOW}WARN:${NC} $1"; }
error() { echo -e "${RED}ERROR:${NC} $1"; exit 1; }
success() { echo -e "${GREEN}SUCCESS:${NC} $1"; }

check_root() {
    if [ "$EUID" -ne 0 ]; then 
        error "Please run as root (sudo ./install.sh)"
    fi
}

check_system() {
    if [ "$(uname)" != "Linux" ]; then
        error "This script only works on Linux systems"
    fi

    if ! command -v systemctl >/dev/null 2>&1; then
        error "Systemd is required but not found"
    fi

    for cmd in dirname readlink; do
        if ! command -v $cmd >/dev/null 2>&1; then
            error "Required command '$cmd' not found"
        fi
    done
}

check_cpu() {
    if [ ! -d "/sys/devices/system/cpu" ]; then
        error "CPU management interface not found"
    fi

    CPU1_ONLINE="/sys/devices/system/cpu/cpu1/online"
    if [ ! -f "$CPU1_ONLINE" ]; then
        warn "CPU core control might not be supported on this system"
    fi
}

backup_config() {
    if [ -f "$CONFIG_FILE" ]; then
        BACKUP_FILE="$CONFIG_FILE.backup.$(date +%Y%m%d_%H%M%S)"
        info "Backing up existing config to $BACKUP_FILE"
        cp "$CONFIG_FILE" "$BACKUP_FILE"
    fi
}

install_files() {
    info "Installing observer..."

    mkdir -p "$CONFIG_DIR"

    info "Installing binary to $BINARY_PATH"
    cp observer "$BINARY_PATH" || error "Failed to install binary"
    chmod +x "$BINARY_PATH"

    if [ ! -f "$CONFIG_FILE" ]; then
        info "Installing default configuration"
        cp config.toml "$CONFIG_FILE" || error "Failed to install config"
    else
        info "Keeping existing configuration"
        info "New default config available at $CONFIG_FILE.new"
        cp config.toml "$CONFIG_FILE.new"
    fi

    info "Installing systemd service"
    cp observer.service "$SERVICE_FILE" || error "Failed to install service"
}

set_permissions() {
    info "Setting permissions..."
    chown -R root:root "$CONFIG_DIR"
    chmod 755 "$CONFIG_DIR"
    chmod 644 "$CONFIG_FILE"
    chmod 644 "$SERVICE_FILE"
}

setup_service() {
    info "Configuring service..."
    systemctl daemon-reload || error "Failed to reload systemd"
    
    if ! systemctl enable observer; then
        error "Failed to enable observer service"
    fi
    
    if ! systemctl start observer; then
        error "Failed to start observer service"
    fi
    
    if ! systemctl is-active --quiet observer; then
        error "Service failed to start. Check 'journalctl -u observer' for details"
    fi
}

print_success() {
    echo
    success "Observer has been successfully installed!"
    echo
    echo "Configuration file: $CONFIG_FILE"
    echo "Service status: $(systemctl is-active observer)"
    echo
    echo "Useful commands:"
    echo "  Check status: systemctl status observer"
    echo "  View logs: journalctl -u observer -f"
    echo "  Edit config: sudo nano $CONFIG_FILE"
    echo
}

cleanup() {
    if [ $? -ne 0 ]; then
        echo
        warn "Installation failed! Cleaning up..."
        [ -f "$BINARY_PATH" ] && rm -f "$BINARY_PATH"
        [ -f "$SERVICE_FILE" ] && rm -f "$SERVICE_FILE"
        systemctl daemon-reload
    fi
}

main() {
    trap cleanup EXIT
    
    echo "Starting Observer installation..."
    echo

    check_root
    check_system
    check_cpu
    backup_config
    install_files
    set_permissions
    setup_service
    print_success
}

main