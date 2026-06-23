#!/usr/bin/env bash
set -e

echo "--- Bezel Smart Installer ---"

INSTALL_MODE="unknown"
NEEDS_RELOGIN=0
BIN_DEST="$HOME/.local/bin/bezel"
REPO_URL="https://github.com/indra55/bezel"
LATEST_RELEASE_URL="$REPO_URL/releases/latest/download/bezel-linux-amd64"
CONFIG_EXAMPLE_URL="https://raw.githubusercontent.com/indra55/bezel/main/config.toml.example"

LOCAL_VERSION="none"
if command -v bezel &> /dev/null; then
    LOCAL_VERSION=$(timeout 1 bezel --version 2>/dev/null | awk '{print $2}')
    if [ -z "$LOCAL_VERSION" ]; then
        LOCAL_VERSION="unknown"
    fi
    BIN_DEST="$(command -v bezel)"
fi

if [ -f "Cargo.toml" ] && grep -q 'name = "bezel"' Cargo.toml 2>/dev/null; then
    INSTALL_MODE="source"
else
    INSTALL_MODE="download"
fi

if [ "$INSTALL_MODE" = "source" ]; then
    echo "[1/6] Local source detected. Building Bezel binary from source..."
    if ! command -v cargo &> /dev/null; then
        echo "Error: Rust/Cargo not found. Install from https://rustup.rs"
        exit 1
    fi
    cargo build --release
    echo "[2/6] Installing binary to ~/.local/bin/bezel..."
    mkdir -p ~/.local/bin
    if [ -f "$BIN_DEST" ]; then
        echo "      Backing up existing binary to $BIN_DEST.bak"
        cp "$BIN_DEST" "$BIN_DEST.bak"
    fi
    cp target/release/bezel ~/.local/bin/
    BIN_DEST="$HOME/.local/bin/bezel"
else
    echo "[1/6] Fetching latest release info..."
    REMOTE_VERSION=$(curl -w "%{url_effective}\n" -I -L -s -S "$REPO_URL/releases/latest" -o /dev/null | awk -F '/' '{print $NF}')
    
    if [ "$LOCAL_VERSION" != "none" ]; then
        if [ "$LOCAL_VERSION" = "$REMOTE_VERSION" ] || [ "v$LOCAL_VERSION" = "$REMOTE_VERSION" ]; then
            echo "      You are already on the latest version ($LOCAL_VERSION). Reinstalling..."
        else
            echo "      Updating $LOCAL_VERSION -> $REMOTE_VERSION..."
        fi
    else
        echo "      Installing version $REMOTE_VERSION..."
    fi

    echo "[2/6] Downloading prebuilt Bezel binary..."
    mkdir -p ~/.local/bin
    
    if [ -f "$BIN_DEST" ]; then
        echo "      Backing up existing binary to $BIN_DEST.bak"
        cp "$BIN_DEST" "$BIN_DEST.bak"
    fi

    if ! curl -sSfL "$LATEST_RELEASE_URL" -o "$HOME/.local/bin/bezel"; then
        echo "      Prebuilt binary not found (release may not exist yet)."
        if command -v cargo &> /dev/null; then
            echo "      Cargo detected. Falling back to source build..."
            cargo install --git "$REPO_URL" --root "$HOME/.local"
            BIN_DEST="$HOME/.local/bin/bezel"
        else
            echo "Error: Failed to download prebuilt binary."
            exit 1
        fi
    else
        chmod +x "$HOME/.local/bin/bezel"
        BIN_DEST="$HOME/.local/bin/bezel"
    fi
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
    echo "You MUST reboot your computer for permissions to apply."
    echo "Once rebooted, the service will start working automatically."
else
    echo "Bezel is now running in the background."
    echo "You can check its status with:"
    echo "  systemctl --user status bezel.service"
fi
echo ""
