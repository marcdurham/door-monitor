# Build

# For RaspberryPi Zero
## Install cross-compilation target
rustup target add arm-unknown-linux-gnueabihf

## Cross-compile for Pi Zero
cargo build --release --target arm-unknown-linux-gnueabihf

## Deploy via SCP
scp target/arm-unknown-linux-gnueabihf/release/door-monitor pi@raspberrypi.local:~/