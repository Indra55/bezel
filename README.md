# Bezel

Bezel is a Linux daemon that provides customizable trackpad edge gestures. It intercepts raw trackpad inputs via libinput/evdev and dispatches shell commands based on directional swipes or taps along the edges (zones) of your trackpad.

## Quick Installation (Recommended)

The easiest way to install and set up Bezel to run automatically in the background is using the provided installation script. 

1. Navigate to the project directory:
   ```sh
   cd bezel
   ```

2. Run the automated installer:
   ```sh
   ./install.sh
   ```

The script will handle compiling the program, adding your user to the necessary groups, setting up the `udev` permissions, and creating a background systemd service.

## Configuration

Bezel uses a configuration file located at `~/.config/bezel/config.toml`. The installer will automatically create a default template for you.

To define a gesture, specify the zone and direction, and the command to run. For example, to bind a top-left swipe to changing workspaces:
```toml
[gestures.top.left]
action = "command"
cmd = "hyprctl dispatch workspace e-1"
```

For a complete working configuration with all default zones, refer to the `config.toml.example` file in this repository.

## Quick Cargo Install (Advanced Testers)

If you know what you are doing and just want the binary compiled and placed in `~/.cargo/bin`, you can skip cloning the repository entirely:
```sh
cargo install --git https://github.com/indra55/bezel
```
*(Note: This skips the install script. You will still need to manually configure the `input` group, `udev` rules, and the background service as detailed below).*

## Advanced / Manual Installation

If you prefer to configure everything manually instead of using `install.sh`, follow these steps:

### 1. Prerequisites
[Install Rust](https://rustup.rs/) to compile the project. You must also have access to the `input` group to read raw device events.

Ensure your user is in the `input` group:
```sh
sudo usermod -aG input $USER
```
*(You will need to log out and log back in for this to take effect).*

To allow the program to create a virtual trackpad for passthrough without requiring root, configure a udev rule for `uinput`:
```sh
echo 'KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### 2. Build and Move Binary
```sh
cargo build --release
mkdir -p ~/.local/bin
cp target/release/bezel ~/.local/bin/
```

### 3. Background Systemd Service
Create a file at `~/.config/systemd/user/bezel.service`:
```ini
[Unit]
Description=Bezel Trackpad Gestures
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/bezel
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
```

Then enable and start it:
```sh
systemctl --user daemon-reload
systemctl --user enable --now bezel.service
```

## Troubleshooting

If the daemon grabs the device exclusively and hard-crashes without releasing it, trackpad movement will stop. If your trackpad stops responding, simply restart the background service:
```sh
systemctl --user restart bezel.service
```
