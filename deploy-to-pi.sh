#!/bin/bash
# deploy-to-pi.sh - Deploy door-monitor to Raspberry Pi Zero

set -e

PI_HOST="${PI_HOST:-pi@raspberrypi.local}"
PI_PATH="${PI_PATH:-/home/pi/door-monitor}"

echo "Building for Raspberry Pi Zero (ARM)..."
cargo build --release --target arm-unknown-linux-gnueabihf

echo "Copying binary to Pi Zero..."
scp target/arm-unknown-linux-gnueabihf/release/door-monitor "$PI_HOST:$PI_PATH/"

echo "Setting up systemd service on Pi..."
ssh "$PI_HOST" << 'EOF'
sudo tee /etc/systemd/system/door-monitor.service > /dev/null << 'SERVICE'
[Unit]
Description=Door Monitor Service
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/door-monitor
ExecStart=/home/pi/door-monitor/door-monitor --api-url http://your-api-url.com
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
SERVICE

sudo systemctl daemon-reload
sudo systemctl enable door-monitor
sudo systemctl start door-monitor
EOF

echo "Deployment complete! Check status with:"
echo "ssh $PI_HOST 'sudo systemctl status door-monitor'"
