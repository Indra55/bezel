# Bezel

Bezel is a Linux daemon that provides customizable trackpad edge gestures.
It intercepts raw trackpad inputs via evdev and dispatches shell
commands based on directional swipes or taps along the edges (zones) of your trackpad.

<p align="center">
  <img width="400" src="demo.png" alt="Bezel Demo">
</p>

<br clear="right">

## Installation

Bezel is Linux-only (Wayland required).

### Prebuilt binary
```sh
curl -sSfL https://raw.githubusercontent.com/indra55/bezel/main/install.sh | bash
```

### From source
```sh
cargo install --git https://github.com/indra55/bezel
```

### Arch Linux

Normally we'd just tell you to `yay -S bezel`, but the AUR is currently experiencing a massive malware apocalypse. Someone adopted 1,500 orphaned packages and turned them into malware, so Arch had to disable new account registrations. We have our `PKGBUILD` ready to go, but until they put out the fire, you'll have to use the prebuilt binary or build from source like a normal person. Stay safe out there!

### Nix
You can run Bezel directly using Nix:
```sh
nix run github:indra55/bezel
```

## Setup

Add yourself to the `input` group (required on all distros):
```sh
sudo usermod -aG input $USER
# log out and back in after this
```

### Configuration
Bezel looks for its configuration at `~/.config/bezel/config.toml`.

To define a gesture, specify the zone and direction, and the command to run:
```toml
[gestures.top.left]
action = "command"
cmd = "hyprctl dispatch workspace e-1"
```
*(See `config.toml.example` in this repo for a complete template).*

### Autostart
Start Bezel when your Wayland compositor starts. **Void Linux / non-systemd** users should also use this method instead of a background service.

For **Hyprland** (`~/.config/hypr/hyprland.conf`):
```conf
exec-once = bezel
```

For **Sway** (`~/.config/sway/config`):
```conf
exec bezel
```

For **Niri** (`~/.config/niri/config.kdl`):
```conf
spawn-at-startup "bezel"
```

## Troubleshooting

If your trackpad stops responding, restart the service:
```sh
systemctl --user restart bezel.service
```
