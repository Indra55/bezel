# Bezel

Bezel is a Linux daemon that provides customizable trackpad edge gestures. It intercepts raw trackpad inputs via libinput/evdev and dispatches shell commands based on directional swipes or taps along the edges (zones) of your trackpad.

## Installation

Bezel features a smart installer that adapts to your preferred installation method. It automatically handles permissions, `udev` rules, and the background `systemd` service.

### 1. Quick Install (Prebuilt Binary)

The easiest way to install Bezel is to download the prebuilt binary and run the setup automatically:

```sh
curl -sSfL https://raw.githubusercontent.com/indra55/bezel/main/install.sh | bash
```

### 2. Cargo Install (From Source)

If you prefer to compile from source but don't want to clone the repository manually, you can use Cargo. The installer will detect the pre-installed binary and skip downloading:

```sh
# 1. Compile and install binary
cargo install --git https://github.com/indra55/bezel

# 2. Run setup for permissions and service
curl -sSfL https://raw.githubusercontent.com/indra55/bezel/main/install.sh | bash
```

### 3. Local Install (Development)

If you are developing or testing local changes:

```sh
# 1. Clone repository
git clone https://github.com/indra55/bezel
cd bezel

# 2. Run local installer
./install.sh
```

## Configuration

Bezel uses a configuration file located at `~/.config/bezel/config.toml`. The installer will automatically create a default template for you.

To define a gesture, specify the zone and direction, and the command to run. For example, to bind a top-left swipe to changing workspaces:
```toml
[gestures.top.left]
action = "command"
cmd = "hyprctl dispatch workspace e-1"
```

For a complete working configuration with all default zones, refer to the `config.toml.example` file in this repository.

## Advanced / Manual Setup

If you prefer to configure everything manually instead of using the installer, follow these steps:

### Prerequisites
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

### Build and Move Binary
```sh
cargo build --release
mkdir -p ~/.local/bin
cp target/release/bezel ~/.local/bin/
```

### Background Systemd Service
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
