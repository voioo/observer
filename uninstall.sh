#!/usr/bin/env bash

# Enable all cores first
for core in /sys/devices/system/cpu/cpu[1-9]*/online; do
    echo 1 | sudo tee "$core" >/dev/null 2>&1
done

# Stop and remove Observer
systemctl stop observer
systemctl disable observer
rm -f /etc/systemd/system/observer.service
rm -f /usr/local/bin/observer
rm -rf /etc/observer
systemctl daemon-reload

echo "Observer has been uninstalled and all cores are enabled"