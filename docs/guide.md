Bezel turns the edges of your Linux trackpad into a configurable control surface. Bind 1–4 fingers to taps, double taps, holds, swipes, and repeating slides. Normal center touches remain available to your desktop.

This guide covers the current `main` branch. Name-based device selection is available on `main` after v1.0.0; it is not included in the v1.0.0 prebuilt binaries. See the [releases](https://github.com/Indra55/bezel/releases) for published versions.

## Getting started

Bezel is Linux-only (Wayland required).

### Prebuilt binary
```sh
curl -sSfL https://raw.githubusercontent.com/indra55/bezel/main/install.sh | bash
```
*(To update to a newer version, simply run this exact same command again. It will safely back up your old binary and seamlessly restart the service.)*

### From source
```sh
cargo install --git https://github.com/indra55/bezel
```

### Arch Linux

Use the prebuilt installer or build from source with Cargo.

### Permissions

Add yourself to the `input` group (required on all distros):
```sh
sudo usermod -aG input $USER
# reboot your computer after this
```

**NixOS Users:** Add `"input"` and `"uinput"` to your `users.users.<name>.extraGroups` instead of using `usermod`.

If you still get `Permission denied (os error 13)` after rebooting, you may need custom udev rules for your physical and virtual trackpads. Create `/etc/udev/rules.d/99-bezel.rules`:
```udev
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_TOUCHPAD}=="1", GROUP="input", MODE="0640"
KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
```
Then reload udev rules with `sudo udevadm control --reload-rules && sudo udevadm trigger`.

**NixOS Users:** set either `services.bezel.enable = true` or `hardware.uinput.enable = true`

## Gestures and configuration

Bezel looks for its configuration at `~/.config/bezel/config.toml`.

On first install, `install.sh` opens **Gesture Studio**. Choose your desktop, then **Customize this edge → finger count → gesture → action**. Assign different controls to the same edge, keep the suggestions with Enter, or clear an edge. Hyprland, Niri, Sway, KDE Plasma, GNOME, and NixOS have logo previews; choosing NixOS asks for your compositor next.

Use arrow keys or type a number, then press Enter. Escape goes back while preserving your choices. Action menus show eight entries per page; keep scrolling or enter an action number directly. Press R to replay an illustrated gesture preview. Optional tuning defaults to Done, so you can skip it with Enter.

The panel uses warm amber accents and previews that match the current step. It needs a UTF-8 terminal at least 80 columns × 24 rows; other terminals use numbered prompts. Terminal state is restored on exit, and shrinking the window switches to the numbered fallback.

To configure a cloned checkout again:

```sh
./onboard.sh
```

Use `--plain` for numbered prompts, `--no-animation` to disable gesture animation, `--no-color` for monochrome, or `--color` to enable accents despite `NO_COLOR`. The plain flow offers basic per-edge choices followed by an advanced multi-finger binding editor.

On upgrades, the installer asks whether to run onboarding again. Existing configurations are kept unless you choose **Apply configuration**, which creates a backup. **Save preview only** writes a separate preview, and Cancel leaves your configuration untouched.

On NixOS, the installer prints a Home Manager snippet instead of creating an unmanaged TOML file or systemd service. Import the Bezel Home Manager module, add the snippet, and configure `hardware.uinput` plus the `input` and `uinput` groups in your NixOS configuration. The snippet includes the gesture command tools (`wpctl`, `brightnessctl`, `playerctl`).

For Arch, Debian/Ubuntu, Fedora, and openSUSE, the same installer uses systemd user services and udev; you need Rust only when building from source or when a prebuilt binary is unavailable. The command tools above must be installed separately.

To define a gesture, specify the zone and direction, and the command to run:
This example uses Hyprland 0.55+ syntax; older Hyprland releases use different dispatch commands.

```toml
[gestures.top.left]
action = "command"
cmd = "hyprctl dispatch 'hl.dsp.focus({ workspace = \"e-1\" })'"
```
See [config.toml.example](https://github.com/Indra55/bezel/blob/main/config.toml.example) for a basic template.

### Multi-finger gestures

For one-finger volume and two-finger brightness on the **same left edge**:

```toml
[gestures.left.up]
action = "command"
cmd = "wpctl set-volume @DEFAULT_SINK@ 5%+"

[gestures.left.down]
action = "command"
cmd = "wpctl set-volume @DEFAULT_SINK@ 5%-"

[[bindings]]
zone = "left"
fingers = 2
gesture = "swipe_up"
action = "command"
cmd = "brightnessctl set 10%+"

[[bindings]]
zone = "left"
fingers = 2
gesture = "swipe_down"
action = "command"
cmd = "brightnessctl set 10%-"
```

Legacy `[gestures.edge.direction]` entries bind one finger. Use `[[bindings]]` for **1–4 fingers**, with `zone`, `fingers`, `gesture`, `action = "command"`, and `cmd`:

```toml
[[bindings]]
zone = "left"
fingers = 2
gesture = "slide_up"
action = "command"
cmd = "wpctl set-volume @DEFAULT_SINK@ 2%+"
step_distance = 0.03
```

Start one finger at an edge, then add the others within **80 ms** (`join_ms`); they may start inside the pad. Existing center touches stay independent. One edge gesture runs at a time, and contacts arriving after the joining window pass through normally. The selected edge and finger count remain fixed until release. Unconfigured counts do not fall back to one-finger commands; gesture contacts remain reserved until lifted. More than four joining contacts cancel the gesture. Your trackpad must report each contact independently using multitouch slots.

Available gestures:

- `tap`, `double_tap`, `hold`.
- `swipe_up`, `swipe_down`, `swipe_left`, `swipe_right`, and `swipe_up_left`, `swipe_up_right`, `swipe_down_left`, `swipe_down_right`.
- `slide_up`, `slide_down`, `slide_left`, `slide_right`: repeat the command for each movement step, with direction reversal supported.

Swipes fire once when all participating fingers lift. Holds fire once while fingers remain down. Slides and holds suppress release gestures. Diagonal bindings apply within 22.5° of their diagonal; otherwise Bezel uses the dominant cardinal direction. Single taps are delayed only when a double tap is bound for the same edge/count; both taps must finish within the double-tap interval and start close together.

Set shared defaults in `[recognition]`. Distances are fractions of the trackpad axes, not physical millimeters:

| Setting | Default | Per-binding override |
| --- | --- | --- |
| `join_ms` | 80 | Shared only |
| `swipe_distance` | 0.05 | Swipes |
| `tap_distance` | 0.02 | Taps and holds |
| `tap_ms` | 200 | Taps and double taps |
| `double_tap_ms` | 300 | Double taps |
| `hold_ms` | 500 | Holds |
| `step_distance` | 0.03 | Slides |

Distances accept 0.001–1; times accept 1–10000 ms (`join_ms`: 1–1000), with joining time no greater than tap/hold time. Explicit bindings override equivalent legacy entries; duplicate explicit bindings and incompatible overrides are rejected. Valid config reloads affect the next gesture; invalid reloads retain the previous configuration.

Pinch/zoom and double-tap swipes are not implemented. Multi-finger recognition requires independent **type-B multitouch slots**; devices that only report an aggregate finger count are unsupported. Input overflow cancels pending gestures and waits for contacts to lift. Normal center contacts remain available through Bezel's virtual trackpad.

### Action presets

Gesture Studio offers up to **30 presets**, with **31** reserved for a custom shell command. The original preset numbers 1–16 are unchanged. Available actions depend on your compositor and installed tools.

| Numbers | Actions |
| --- | --- |
| 1–3, 11 | Volume up/down, mute speakers, mute microphone |
| 4–5 | Brightness up/down |
| 6–10 | Play/pause, next/previous track, seek forward/backward 10 seconds |
| 12–15 | Previous/next workspace, move window to previous/next workspace |
| 16 | Toggle Hyprland magic workspace |
| 17–19 | Close focused window, toggle fullscreen, toggle floating |
| 20–23 | Focus window left/right/up/down |
| 24 | Open application launcher |
| 25 | Open terminal |
| 26 | Lock screen |
| 27–28 | Screenshot selected region or entire screen |
| 29 | Open clipboard history |
| 30 | Toggle Do Not Disturb |
| 31 | Custom shell command |

Window and workspace presets use commands for Hyprland, Niri, or Sway. Hyprland presets use the Lua dispatcher syntax for 0.55+; older releases can use custom commands. Plasma, GNOME, and generic desktops omit unsupported window/workspace choices.

Utility presets detect fuzzel/wofi/rofi, common terminals, hyprlock/swaylock/loginctl, grim + slurp + wl-copy (or Niri's screenshot UI), cliphist + a picker + wl-copy, and swaync/dunst. grim screenshots are copied to the clipboard; Niri screenshots use Niri's configured saving behavior. Clipboard history needs an existing cliphist capture service. Nix users should add the tools they select to their packages.

See [config.toml.example](https://github.com/Indra55/bezel/blob/main/config.toml.example) and [config.nix.example](https://github.com/Indra55/bezel/blob/main/config.nix.example) for templates.

## NixOS and Home Manager

You can try out Bezel using this command:
```sh
nix run github:indra55/bezel
```

For a permanent installation, first add Bezel to your flake inputs:
```nix
{
  inputs = {
    # ... other inputs
    bezel = {
      url = "github:Indra55/bezel";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # ... rest of your flake
}
```

Also make sure that you have `extraSpecialArgs = { inherit inputs; };` in your flake outputs.

Bezel provides a NixOS module. To enable it:
```nix
{ inputs, ... }: {
  imports = [
    inputs.bezel.nixosModules.default
  ];
}
```

You can now enable the Bezel service. This snippet will:
1. install the Bezel package on your system;
2. create and enable the Bezel service for all users;
3. configure udev rules.
**NOTE:** you'll still have to add yourself to the `input` and `uinput` groups
```nix
{ ... }: {
  services.bezel.enable = true;
}
```

If you prefer to enable Bezel per-user instead, you can do so using Home Manager.
```nix
{ inputs, ... }: {
  imports = [
    inputs.bezel.homeModules.default
  ];

  services.bezel.enable = true;
}
```

`homeManagerModules.default` remains available for existing configurations.

**NOTE:** If you use the Home Manager module, you'll have to enable uinput separately in your NixOS config, as Home Manager doesn't have access to them:
```nix
{ ... }: {
  hardware.uinput.enable = true;
}
```

After importing the Home Manager module, configure gestures with `services.bezel.config`. See [config.nix.example](https://github.com/Indra55/bezel/blob/main/config.nix.example) for a complete template. Add the command tools used by your gestures to your packages.

## Autostart and OSD

### On-screen notifications

```toml
[osd]
enabled = true
backend = "notify-send" # Valid options: "notify-send", "swayosd", "pipe"
canonical_hints = false # Set to true only if using mako or notify-osd
```

### Starting with your compositor

> **Warning:** If you used the NixOS/HM module or `install.sh` script, Bezel is already running as a `systemd` service! **Do not** add these autostart commands, or you will run two instances simultaneously and they will crash.

Start Bezel when your Wayland compositor starts. **Void Linux / non-systemd** users should use this method instead of a background service. If `bezel` is not in your system `$PATH`, use the absolute path `~/.local/bin/bezel` (or `~/.cargo/bin/bezel` if built with cargo).

For **Hyprland 0.55+** (`~/.config/hypr/hyprland.lua`):
```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("~/.local/bin/bezel")
end)
```

For **older Hyprland** (`~/.config/hypr/hyprland.conf`):
```conf
exec-once = ~/.local/bin/bezel
```

For **Sway** (`~/.config/sway/config`):
```conf
exec ~/.local/bin/bezel
```

For **Niri** (`~/.config/niri/config.kdl`):
```conf
spawn-at-startup "~/.local/bin/bezel"
```

### Web Visualizer (Demo)

Bezel includes a web-based visualizer that shows a graphical representation of your trackpad and animates when you perform gestures. It's useful for testing, debugging, or recording demos.

To use the visualizer:
1. Update your `~/.config/bezel/config.toml` to use the `pipe` OSD backend:
   ```toml
   [osd]
   enabled = true
   backend = "pipe"
   ```
2. Restart Bezel (`systemctl --user restart bezel.service` or restart manually).
3. Start the visualization server from the root of this repository:
   ```sh
   python3 viz/server.py
   ```
4. Open `http://localhost:8080` in your web browser. When you perform gestures, the server will read them from the pipe (`/tmp/bezel-osd`) and broadcast them to the webpage in real-time.

## Troubleshooting

### Compatibility with libinput-gestures

If you are running both Bezel and `libinput-gestures`, you may find that `libinput-gestures` stops working when Bezel is active. This happens because Bezel grabs the physical trackpad exclusively, and `libinput-gestures`' auto-detection heuristic prefers devices with "touchpad" in the name over "trackpad". It will bind to the silenced hardware device instead of the virtual one created by Bezel.

To fix this, you must explicitly tell `libinput-gestures` to use Bezel's virtual device by adding the following line to your `/etc/libinput-gestures.conf` (or `~/.config/libinput-gestures.conf` if you're using a per-user config):
```conf
device Bezel Virtual Trackpad
```
Then restart `libinput-gestures` (`libinput-gestures-setup restart`).
### OSD Notifications Not Showing

If OSD notifications (`notify-send`) fail or don't show up when Bezel is run as a `systemd` service, your compositor might not be exporting the D-Bus environment properly. You can check the logs to see if `notify-send` is exiting with an error. 

To fix this, add the following to your compositor's startup config to import the session variables:

**Hyprland 0.55+** (`hyprland.lua`):
```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP DBUS_SESSION_BUS_ADDRESS")
end)
```

**Older Hyprland** (`hyprland.conf`):
```conf
exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP DBUS_SESSION_BUS_ADDRESS
```

**Sway:**
```conf
exec systemctl --user import-environment WAYLAND_DISPLAY SWAYSOCK DBUS_SESSION_BUS_ADDRESS
```
*(Niri does this automatically).*
### Selecting the correct device

If Bezel accidentally grabs a touchscreen or stylus instead of your trackpad, you can bypass auto-detection by explicitly defining the trackpad's name in your `~/.config/bezel/config.toml`.

You can find your trackpad's exact name by running `sudo libinput list-devices` and copying the string from the **Device** line.

```toml
[device]
path = "PIXA3854:00 093A:0239 Touchpad"
```

*(Note: Using the device name is recommended over hardcoding a `/dev/input/event*` path, as event numbers change on reboot.)*

Restart the service for the changes to take effect:

```sh
systemctl --user restart bezel.service
```
### Debugging Logs

To enable more detailed logging, you can set the `BEZEL_LOG` environment variable. Valid log levels are `error`, `warn`, `info`, `debug`, and `trace`. For example:
```sh
BEZEL_LOG=debug bezel
```
If you are running Bezel as a systemd service, you can run `systemctl --user edit bezel.service` and add the following to the override file:
```ini
[Service]
Environment="BEZEL_LOG=debug"
```
Then restart the service.

If your trackpad stops responding, restart the service:
```sh
systemctl --user restart bezel.service
```
