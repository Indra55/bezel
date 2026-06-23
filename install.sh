#!/usr/bin/env bash
set -e

echo "--- Bezel Installer ---"

if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Install from https://rustup.rs"
    exit 1
fi

if [ ! -f "Cargo.toml" ]; then
    echo "Error: Run this script from the project root directory."
    exit 1
fi

# 1. Build the release binary
echo "[1/6] Building Bezel binary (this might take a minute)..."
cargo build --release

# 2. Install binary
echo "[2/6] Installing binary to ~/.local/bin/bezel..."
mkdir -p ~/.local/bin
cp target/release/bezel ~/.local/bin/

if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "      NOTE: ~/.local/bin is not in your PATH. Add it to your shell profile."
fi

# 3. Setup Default Config Template
echo "[3/6] Setting up default configuration template..."
mkdir -p ~/.config/bezel
if [ ! -f ~/.config/bezel/config.toml ]; then
    cp config.toml.example ~/.config/bezel/config.toml
    echo "      Created default config at ~/.config/bezel/config.toml"
else
    echo "      Config already exists at ~/.config/bezel/config.toml (skipping)"
fi

# 4. Setup Udev rules
echo "[4/6] Setting up udev rules for /dev/uinput..."
echo "      (You may be prompted for your sudo password)"
echo 'KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules > /dev/null
sudo udevadm control --reload-rules && sudo udevadm trigger

# 5. Check Input Group
echo "[5/6] Checking input group permissions..."
if ! groups $USER | grep -q "\binput\b"; then
    echo "      Adding $USER to the 'input' group..."
    sudo usermod -aG input $USER
    NEEDS_RELOGIN=1
else
    echo "      User $USER is already in the 'input' group."
    NEEDS_RELOGIN=0
fi

# 6. Setup Systemd Service
echo "[6/6] Configuring background service..."
mkdir -p ~/.config/systemd/user/
cat << 'EOF' > ~/.config/systemd/user/bezel.service
[Unit]
Description=Bezel Trackpad Gestures
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/bezel
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable bezel.service

if [ "$NEEDS_RELOGIN" -eq 0 ]; then
    systemctl --user start bezel.service
fi

echo ""
echo "--- Install Complete ---"
if [ "$NEEDS_RELOGIN" -eq 1 ]; then
    echo "WARNING: You were just added to the 'input' group."
    echo "You MUST log out and log back in for permissions to apply."
    echo "Once logged back in, the service will start working automatically."
else
    echo "Bezel is now running in the background."
    echo "You can check its status with:"
    echo "  systemctl --user status bezel.service"
fi
echo ""
