# Build

# For RaspberryPi Zero
## Install cross-compilation target
rustup target add arm-unknown-linux-gnueabihf

## Cross-compile for Pi Zero
cargo build --release --target arm-unknown-linux-gnueabihf

## Deploy via SCP
scp target/arm-unknown-linux-gnueabihf/release/door-monitor pi@raspberrypi.local:~/

# Uninstalling Rust on RaspberryPi Zero W v1.1 (Zero/1)
It's been a challenge building this project for the correct target,
I keep getting "Illegal instruction" and the target is weird,
Pi Zero/1 uses ARMv6 I guess instead of v7.

So I'm trying to install Rust on the Pi Zero, naturally it's super slow.

To uninstall it you can type:
```bash
rustup self uninstall
```

I might need to do that if I ever figure out how to build for the correct target.

