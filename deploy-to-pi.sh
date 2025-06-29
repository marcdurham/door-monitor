#!/bin/bash

# Door Monitor Deployment Script
# Builds and deploys to Raspberry Pi devices

set -e

# Configuration
DEFAULT_PI_USER="$USER"
DEFAULT_PI_HOST=""
DEFAULT_TARGET="arm-unknown-linux-gnueabihf"  # Pi Zero/1 default
BINARY_NAME="door-monitor"
REMOTE_PATH="/home/$DEFAULT_PI_USER/$BINARY_NAME"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -h, --host HOST       Raspberry Pi hostname or IP address"
    echo "  -u, --user USER       SSH username (default: pi)"
    echo "  -t, --target TARGET   Rust target:"
    echo "                          arm-unknown-linux-gnueabihf (Pi Zero/1)"
    echo "                          aarch64-unknown-linux-gnu (Pi 4/5)"
    echo "  -r, --release         Build in release mode"
    echo "  -b, --build-only      Only build, don't deploy"
    echo "  --help                Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 -h raspberrypi.local                # Deploy to Pi Zero/1"
    echo "  $0 -h 192.168.1.100 -t aarch64-unknown-linux-gnu  # Deploy to Pi 4/5"
    echo "  $0 -h mypi.local -r                   # Release build and deploy"
    echo "  $0 -b                                  # Build for all targets"
}

# Parse command line arguments
BUILD_MODE="debug"
BUILD_ONLY=false
PI_HOST=""
PI_USER="$DEFAULT_PI_USER"
TARGET="$DEFAULT_TARGET"

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--host)
            PI_HOST="$2"
            shift 2
            ;;
        -u|--user)
            PI_USER="$2"
            shift 2
            ;;
        -t|--target)
            TARGET="$2"
            shift 2
            ;;
        -r|--release)
            BUILD_MODE="release"
            shift
            ;;
        -b|--build-only)
            BUILD_ONLY=true
            shift
            ;;
        --help)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

echo -e "${GREEN}🔨 Door Monitor Deployment Script${NC}"

# Validate target
case $TARGET in
    arm-unknown-linux-gnueabihf)
        echo "📟 Building for Raspberry Pi Zero/1 (32-bit ARM)"
        ;;
    aarch64-unknown-linux-gnu)
        echo "📟 Building for Raspberry Pi 4/5 (64-bit ARM)"
        ;;
    *)
        echo -e "${RED}❌ Unsupported target: $TARGET${NC}"
        echo "Supported targets: arm-unknown-linux-gnueabihf, aarch64-unknown-linux-gnu"
        exit 1
        ;;
esac

# Check if cargo and cross-compilation toolchain are available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo not found. Please install Rust.${NC}"
    exit 1
fi

# Check if target is installed
if ! rustup target list --installed | grep -q "$TARGET"; then
    echo -e "${YELLOW}⚠️  Target $TARGET not installed. Installing...${NC}"
    rustup target add "$TARGET"
fi

# Build for specified target or all targets if build-only
if [[ "$BUILD_ONLY" == true ]]; then
    echo -e "${GREEN}🔨 Building for all supported targets${NC}"
    
    # Build for Pi Zero/1 (32-bit ARM)
    echo "Building for Pi Zero/1..."
    if [[ "$BUILD_MODE" == "release" ]]; then
        cross build --target arm-unknown-linux-gnueabihf --release
    else
        cargo build --target arm-unknown-linux-gnueabihf
    fi
    
    # Build for Pi 4/5 (64-bit ARM)
    echo "Building for Pi 4/5..."
    if [[ "$BUILD_MODE" == "release" ]]; then
        cargo build --target aarch64-unknown-linux-gnu --release
    else
        cargo build --target aarch64-unknown-linux-gnu
    fi
    
    echo -e "${GREEN}✅ Build completed for all targets${NC}"
    echo ""
    echo "Built binaries:"
    find target -name "$BINARY_NAME" -type f -exec file {} \;
    exit 0
fi

# For deployment, require host
if [[ -z "$PI_HOST" ]]; then
    echo -e "${RED}❌ Pi hostname/IP required for deployment. Use -h option.${NC}"
    print_usage
    exit 1
fi

# Build the binary
echo -e "${GREEN}🔨 Building $BINARY_NAME ($BUILD_MODE mode) for $TARGET${NC}"
if [[ "$BUILD_MODE" == "release" ]]; then
    cross build --target "$TARGET" --release
    BINARY_PATH="target/$TARGET/release/$BINARY_NAME"
else
    cargo build --target "$TARGET"
    BINARY_PATH="target/$TARGET/debug/$BINARY_NAME"
fi

# Check if binary was built successfully
if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}❌ Build failed. Binary not found at $BINARY_PATH${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build completed${NC}"

# Test SSH connection
echo -e "${GREEN}🔗 Testing SSH connection to $PI_USER@$PI_HOST${NC}"
if ! ssh -o ConnectTimeout=10 -o BatchMode=yes "$PI_USER@$PI_HOST" 'echo "SSH connection successful"' &>/dev/null; then
    echo -e "${RED}❌ Cannot connect to $PI_USER@$PI_HOST${NC}"
    echo "Please ensure:"
    echo "  1. The Pi is powered on and connected to network"
    echo "  2. SSH is enabled on the Pi"
    echo "  3. Your SSH key is set up or you can use password authentication"
    exit 1
fi

echo -e "${GREEN}✅ SSH connection successful${NC}"

# Stop any running instance
echo -e "${GREEN}🛑 Stopping any running door-monitor instances${NC}"
echo "Debug: Attempting to stop processes matching '$BINARY_NAME' on $PI_USER@$PI_HOST"

# First, check if any processes are running
if ssh "$PI_USER@$PI_HOST" "pgrep -f '$BINARY_NAME'" >/dev/null 2>&1; then
    echo "Found running processes, attempting to stop them..."
    if ssh "$PI_USER@$PI_HOST" "sudo pkill -f '$BINARY_NAME'"; then
        echo "✅ Successfully stopped running processes"
        # Give processes time to terminate gracefully
        sleep 2
    else
        echo "⚠️  Failed to stop processes gracefully, trying force kill..."
        ssh "$PI_USER@$PI_HOST" "sudo pkill -9 -f '$BINARY_NAME'" || true
    fi
else
    echo "✅ No running processes found (this is normal)"
fi

# Copy binary to Pi
echo -e "${GREEN}📤 Copying binary to $PI_HOST${NC}"
scp "$BINARY_PATH" "$PI_USER@$PI_HOST:$REMOTE_PATH"

# Make binary executable
echo -e "${GREEN}🔐 Making binary executable${NC}"
ssh "$PI_USER@$PI_HOST" "chmod +x $REMOTE_PATH"

# Create systemd service file
echo -e "${GREEN}⚙️ Creating systemd service${NC}"
ssh "$PI_USER@$PI_HOST" "sudo tee /etc/systemd/system/door-monitor.service > /dev/null" << EOF
[Unit]
Description=Door Monitor Service
After=network.target
Wants=network.target

[Service]
Type=simple
User=$PI_USER
WorkingDirectory=/home/$PI_USER
ExecStart=/home/$PI_USER/run-door-monitor.sh
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Create example config file if it doesn't exist
echo -e "${GREEN}📋 Creating example config file${NC}"
ssh "$PI_USER@$PI_HOST" "test -f /home/$PI_USER/door-monitor-config.json || cat > /home/$PI_USER/door-monitor-config.json" << 'EOF'
{
  "door_url": "http://your-door-sensor.local/status",
  "poll_interval_seconds": 30,
  "open_threshold_seconds": 300,
  "sms": {
    "account_sid": "your_twilio_account_sid",
    "auth_token": "your_twilio_auth_token",
    "from_number": "+1234567890",
    "to_number": "+0987654321"
  }
}
EOF

# Reload systemd and enable service
echo -e "${GREEN}🔄 Configuring systemd service${NC}"
ssh "$PI_USER@$PI_HOST" "sudo systemctl daemon-reload"
ssh "$PI_USER@$PI_HOST" "sudo systemctl enable door-monitor"

echo -e "${GREEN}🚀 Deployment completed successfully!${NC}"
echo ""
echo "Next steps:"
echo "1. Edit the config file: ssh $PI_USER@$PI_HOST 'nano door-monitor-config.json'"
echo "2. Start the service: ssh $PI_USER@$PI_HOST 'sudo systemctl start door-monitor'"
echo "3. Check status: ssh $PI_USER@$PI_HOST 'sudo systemctl status door-monitor'"
echo "4. View logs: ssh $PI_USER@$PI_HOST 'sudo journalctl -u door-monitor -f'"
