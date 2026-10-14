# Bezel

Bezel turns the edges of your Linux trackpad into a configurable control surface. Bind **1–4 fingers** independently: use one finger on the left edge for volume, two for brightness, and three for workspace controls.

https://github.com/user-attachments/assets/f56bbd12-6394-4726-b41b-ba38439b61e5

## Features

- Taps, double taps, holds, eight swipe directions, and repeating slides.
- Gesture Studio: a terminal wizard with gesture previews, 30 action presets, and custom commands.
- Different bindings for each edge and finger count, with shared or per-binding sensitivity.
- NixOS and Home Manager support, plus compatible legacy one-finger configuration.
- Normal center touches remain available to your desktop.

Bezel requires Linux, Wayland, and a trackpad with type-B multitouch slots. Pinch/zoom and tap-then-swipe gestures are not implemented.

## Install

```sh
curl -sSfL https://raw.githubusercontent.com/indra55/bezel/main/install.sh | bash
```

Run the same command to update. On first install, Gesture Studio helps you configure gestures and set up the service. Command tools such as `wpctl`, `brightnessctl`, and `playerctl` must be installed separately.

From source:

```sh
cargo install --git https://github.com/indra55/bezel
```

For NixOS and Home Manager, see the [installation guide](https://hitanshu.xyz/bezel/docs#nixos-and-home-manager).

## First setup

Add your user to the `input` group and reboot:

```sh
sudo usermod -aG input "$USER"
```

On NixOS, configure the `input` and `uinput` groups declaratively. See the guide for permissions and udev setup.

Bezel reads `~/.config/bezel/config.toml`. To run Gesture Studio again from a cloned checkout:

```sh
./onboard.sh
```

The NixOS package also installs the onboard script under the `bezel-onboard` command.

A two-finger swipe on the left edge can control brightness:

```toml
[[bindings]]
zone = "left"
fingers = 2
gesture = "swipe_up"
action = "command"
cmd = "brightnessctl set 10%+"
```

If the installer or a Nix module already runs Bezel as a service, do not start another instance from your compositor.

## Documentation

Read the [full documentation](https://hitanshu.xyz/bezel/docs) for:

- Installation, permissions, and first setup.
- Gesture bindings, presets, and sensitivity settings.
- NixOS and Home Manager configuration.
- Autostart, on-screen notifications, and the web visualizer.
- Device selection and troubleshooting.

The same guide is available [in this repository](docs/guide.md). Configuration templates: [TOML](config.toml.example) and [Nix](config.nix.example).

The guide describes the current `main` branch. Check [releases](https://github.com/Indra55/bezel/releases) for published binaries.

## Contributing

[Report a bug or suggest a feature](https://github.com/Indra55/bezel/issues). Include your Bezel version, compositor, trackpad model, and relevant logs when reporting a problem. Documentation improvements belong in [docs/guide.md](docs/guide.md).

## License

[GPL-3.0-or-later](LICENSE).
