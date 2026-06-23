#!/usr/bin/env bash
set -e

echo "--- Bezel Smart Installer ---"

INSTALL_MODE="unknown"
BIN_DEST="$HOME/.local/bin/bezel"
REPO_URL="https://github.com/indra55/bezel"
LATEST_RELEASE_URL="$REPO_URL/releases/latest/download/bezel-linux-amd64"
CONFIG_EXAMPLE_URL="https://raw.githubusercontent.com/indra55/bezel/main/config.toml.example"

if [ -f "Cargo.toml" ] && grep -q 'name = "bezel"' Cargo.toml 2>/dev/null; then
    INSTALL_MODE="source"
elif command -v bezel &> /dev/null; then
    INSTALL_MODE="preinstalled"
    BIN_DEST="$(command -v bezel)"
else
    INSTALL_MODE="download"
fi

if [ "$INSTALL_MODE" = "source" ]; then
    if ! command -v cargo &> /dev/null; then
        echo "Error: Rust/Cargo not found. Install from https://rustup.rs"
        exit 1
    fi
    echo "[1/6] Building Bezel binary from source (this might take a minute)..."
    cargo build --release
    echo "[2/6] Installing binary to ~/.local/bin/bezel..."
    mkdir -p ~/.local/bin
    cp target/release/bezel ~/.local/bin/
    BIN_DEST="$HOME/.local/bin/bezel"
elif [ "$INSTALL_MODE" = "preinstalled" ]; then
    echo "[1/6] Found existing Bezel binary at $BIN_DEST. Skipping build."
    echo "[2/6] Skipping binary installation."
elif [ "$INSTALL_MODE" = "download" ]; then
    echo "[1/6] Downloading prebuilt Bezel binary..."
    mkdir -p ~/.local/bin
    if ! curl -sSfL "$LATEST_RELEASE_URL" -o "$HOME/.local/bin/bezel"; then
        echo "Error: Failed to download prebuilt binary. The release might not exist yet."
        echo "Please compile from source using: cargo install --git $REPO_URL"
        exit 1
    fi
    chmod +x "$HOME/.local/bin/bezel"
    echo "[2/6] Installed binary to ~/.local/bin/bezel"
    BIN_DEST="$HOME/.local/bin/bezel"
fi

if [[ ":$PATH:" != *":$(dirname "$BIN_DEST"):"* ]]; then
    echo "      NOTE: $(dirname "$BIN_DEST") is not in your PATH. Add it to your shell profile."
fi

# 3. Setup Default Config Template
echo "[3/6] Setting up default configuration template..."
mkdir -p ~/.config/bezel
if [ ! -f ~/.config/bezel/config.toml ]; then
    if [ -f "config.toml.example" ]; then
        cp config.toml.example ~/.config/bezel/config.toml
    else
        curl -sSfL "$CONFIG_EXAMPLE_URL" -o ~/.config/bezel/config.toml || echo "      Warning: Could not download config template."
    fi
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
cat << EOF > ~/.config/systemd/user/bezel.service
[Unit]
Description=Bezel Trackpad Gestures
After=graphical-session.target

[Service]
ExecStart=$BIN_DEST
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable bezel.service

if [ "$NEEDS_RELOGIN" -eq 0 ]; then
    # Don't fail the install if the service start fails (e.g. binary not yet functional)
    systemctl --user start bezel.service || true
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
