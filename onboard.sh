#!/usr/bin/env bash
# Interactive first-run configuration. The installer uses --if-missing on upgrades.
set -euo pipefail

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/bezel"
config_file="$config_dir/config.toml"
desktop_explicit=0
if_missing=0
declarative=0
color_mode=auto

detect_desktop() {
    local session="${XDG_CURRENT_DESKTOP:-${XDG_SESSION_DESKTOP:-}}"
    session="${session,,}"
    case "$session" in
        *hyprland*) echo hyprland ;;
        *niri*) echo niri ;;
        *sway*) echo sway ;;
        *kde*|*plasma*) echo plasma ;;
        *gnome*) echo gnome ;;
        *)
            if [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then echo hyprland
            elif [ -n "${NIRI_SOCKET:-}" ]; then echo niri
            elif [ -n "${SWAYSOCK:-}" ]; then echo sway
            else echo generic; fi
            ;;
    esac
}

desktop="$(detect_desktop)"
if [ -f /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    if [ "${ID:-}" = nixos ]; then declarative=1; fi
fi

while [ "$#" -gt 0 ]; do
    case "$1" in
        --desktop)
            [ "$#" -ge 2 ] || { echo "--desktop needs a value" >&2; exit 2; }
            desktop="$2"; desktop_explicit=1; shift 2 ;;
        --nixos) declarative=1; shift ;;
        --toml) declarative=0; shift ;;
        --color) color_mode=always; shift ;;
        --no-color) color_mode=never; shift ;;
        --if-missing) if_missing=1; shift ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

case "$desktop" in
    hyprland|niri|sway|plasma|gnome|generic) ;;
    *) echo "Unsupported desktop: $desktop" >&2; exit 2 ;;
esac

if [ "$if_missing" -eq 1 ] && [ "$declarative" -eq 0 ] && { [ -e "$config_file" ] || [ -L "$config_file" ]; }; then
    echo "Existing config kept at $config_file"
    exit 0
fi

input_source="none"
if [ -t 0 ]; then
    input_source="stdin"
elif { : </dev/tty; } 2>/dev/null; then
    input_source="tty"
fi

ui_accent='' ui_soft='' ui_halo='' ui_faint='' ui_reset=''
if [ "$color_mode" = always ] || { [ "$color_mode" = auto ] && [ -t 2 ] && [ -z "${NO_COLOR:-}" ]; }; then
    ui_accent=$'\033[38;2;199;139;95m'
    ui_soft=$'\033[38;2;161;115;83m'
    ui_halo=$'\033[38;2;98;75;59m'
    ui_faint=$'\033[38;2;65;55;47m'
    ui_reset=$'\033[0m'
fi

ask() {
    local prompt="$1"
    printf '%s' "$prompt" >&2
    case "$input_source" in
        stdin) IFS= read -r answer || answer="" ;;
        tty) IFS= read -r answer </dev/tty || answer="" ;;
        none) answer=""; printf '\n' >&2 ;;
    esac
}

printf '%b' "$ui_soft" >&2
cat >&2 <<'ART'

░▒▓███████▓▒░░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░             ░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░           ░▒▓██▓▒░░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓███████▓▒░░▒▓██████▓▒░    ░▒▓██▓▒░  ░▒▓██████▓▒░ ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░       ░▒▓██▓▒░    ░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░      ░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓███████▓▒░░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░
ART
printf '%b\n' "$ui_reset" >&2
printf '%bWELCOME TO BEZEL.%b\n' "$ui_accent" "$ui_reset" >&2
printf 'FOUR EDGES. YOUR RULES. ZERO WASTED MOTION.\n' >&2
printf 'The fine highlight marks the active edge.\n' >&2

desktop_name() {
    case "$1" in
        hyprland) echo Hyprland ;; niri) echo Niri ;; sway) echo Sway ;;
        plasma) echo 'KDE Plasma' ;; gnome) echo GNOME ;; generic) echo 'Other Wayland desktop' ;;
    esac
}

if [ "$desktop_explicit" -eq 0 ]; then
    printf '\nSession detected: %s\n' "$(desktop_name "$desktop")" >&2
    printf '  1) Hyprland   2) Niri   3) Sway\n' >&2
    printf '  4) KDE Plasma 5) GNOME  6) Other Wayland\n' >&2
    while :; do
        ask "Pick your desktop [Enter = $(desktop_name "$desktop")]: "
        case "$answer" in
            '') break ;; 1) desktop=hyprland; break ;; 2) desktop=niri; break ;;
            3) desktop=sway; break ;; 4) desktop=plasma; break ;;
            5) desktop=gnome; break ;; 6) desktop=generic; break ;;
            *) echo 'Choose 1-6, or press Enter.' >&2 ;;
        esac
    done
fi

workspace_prev=""; workspace_next=""; move_prev=""; move_next=""
case "$desktop" in
    hyprland)
        workspace_prev='hyprctl dispatch workspace e-1'
        workspace_next='hyprctl dispatch workspace e+1'
        move_prev='hyprctl dispatch movetoworkspace e-1'
        move_next='hyprctl dispatch movetoworkspace e+1' ;;
    niri)
        workspace_prev='niri msg action focus-workspace-up'
        workspace_next='niri msg action focus-workspace-down'
        move_prev='niri msg action move-window-to-workspace-up'
        move_next='niri msg action move-window-to-workspace-down' ;;
    sway)
        workspace_prev='swaymsg workspace prev'
        workspace_next='swaymsg workspace next'
        move_prev='swaymsg move container to workspace prev'
        move_next='swaymsg move container to workspace next' ;;
esac

declare -A bindings=(
    [left.up]=1 [left.down]=2 [left.tap]=3
    [right.up]=4 [right.down]=5
    [bottom.left]=8 [bottom.right]=7 [bottom.tap]=6
    [bottom.up]=10 [bottom.down]=9
)
if [ -n "$workspace_prev" ]; then
    bindings[top.left]=12
    bindings[top.right]=13
fi

action_name() {
    case "$1" in
        0) echo 'Unused' ;; 1) echo 'Volume up' ;; 2) echo 'Volume down' ;;
        3) echo 'Mute speakers' ;; 4) echo 'Brightness up' ;;
        5) echo 'Brightness down' ;; 6) echo 'Play / pause' ;;
        7) echo 'Next track' ;; 8) echo 'Previous track' ;;
        9) echo 'Seek forward 10s' ;; 10) echo 'Seek backward 10s' ;;
        11) echo 'Mute microphone' ;; 12) echo 'Previous workspace' ;;
        13) echo 'Next workspace' ;; 14) echo 'Move window to previous workspace' ;;
        15) echo 'Move window to next workspace' ;;
    esac
}

action_command() {
    case "$1" in
        1) echo 'wpctl set-volume @DEFAULT_SINK@ 5%+' ;;
        2) echo 'wpctl set-volume @DEFAULT_SINK@ 5%-' ;;
        3) echo 'wpctl set-mute @DEFAULT_SINK@ toggle' ;;
        4) echo 'brightnessctl set 10%+' ;;
        5) echo 'brightnessctl set 10%-' ;;
        6) echo 'playerctl play-pause' ;;
        7) echo 'playerctl next' ;;
        8) echo 'playerctl previous' ;;
        9) echo 'playerctl position 10+' ;;
        10) echo 'playerctl position 10-' ;;
        11) echo 'wpctl set-mute @DEFAULT_SOURCE@ toggle' ;;
        12) echo "$workspace_prev" ;; 13) echo "$workspace_next" ;;
        14) echo "$move_prev" ;; 15) echo "$move_next" ;;
    esac
}

draw_trackpad() {
    local thin='────────────────────────────'
    local bold='━━━━━━━━━━━━━━━━━━━━━━━━━━━━'
    local haze='┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈'
    local empty='                            '
    local left_arrow='  ←                         '
    local right_arrow='                         →  '
    local top_arrow='             ↑              '
    local bottom_arrow='             ↓              '
    local row
    printf '\n  %b%02d / %s EDGE%b\n' "$ui_accent" "$step" "${1^^}" "$ui_reset" >&2
    case "$1" in
        top)
            printf '         %b%s%b\n' "$ui_halo" "$haze" "$ui_reset" >&2
            printf '        %b╭%s╮%b\n' "$ui_accent" "$bold" "$ui_reset" >&2 ;;
        *) printf '        ╭%s╮\n' "$thin" >&2 ;;
    esac
    for ((row = 0; row < 7; row++)); do
        case "$1" in
            left)
                printf '      %b┆%b%b┊%b%b┃%b' "$ui_faint" "$ui_reset" "$ui_halo" "$ui_reset" "$ui_accent" "$ui_reset" >&2
                if [ "$row" -eq 3 ]; then
                    printf '%b%s%b' "$ui_accent" "$left_arrow" "$ui_reset" >&2
                else
                    printf '%s' "$empty" >&2
                fi
                printf '│\n' >&2 ;;
            right)
                printf '        │' >&2
                if [ "$row" -eq 3 ]; then
                    printf '%b%s%b' "$ui_accent" "$right_arrow" "$ui_reset" >&2
                else
                    printf '%s' "$empty" >&2
                fi
                printf '%b┃%b%b┊%b%b┆%b\n' "$ui_accent" "$ui_reset" "$ui_halo" "$ui_reset" "$ui_faint" "$ui_reset" >&2 ;;
            top)
                if [ "$row" -eq 0 ]; then
                    printf '        │%b%s%b│\n' "$ui_accent" "$top_arrow" "$ui_reset" >&2
                else
                    printf '        │%s│\n' "$empty" >&2
                fi ;;
            bottom)
                if [ "$row" -eq 6 ]; then
                    printf '        │%b%s%b│\n' "$ui_accent" "$bottom_arrow" "$ui_reset" >&2
                else
                    printf '        │%s│\n' "$empty" >&2
                fi ;;
        esac
    done
    case "$1" in
        bottom)
            printf '        %b╰%s╯%b\n' "$ui_accent" "$bold" "$ui_reset" >&2
            printf '         %b%s%b\n' "$ui_halo" "$haze" "$ui_reset" >&2 ;;
        *) printf '        ╰%s╯\n' "$thin" >&2 ;;
    esac
}

show_actions() {
    local id
    printf '\nPick what a gesture does:\n  0) Leave unused\n' >&2
    for id in {1..11}; do
        printf ' %2d) %s\n' "$id" "$(action_name "$id")" >&2
    done
    if [ -n "$workspace_prev" ]; then
        for id in {12..15}; do
            printf ' %2d) %s\n' "$id" "$(action_name "$id")" >&2
        done
    fi
}

step=0
for edge in left right top bottom; do
    step=$((step + 1))
    draw_trackpad "$edge"
    printf 'Suggested bindings:\n' >&2
    for direction in up down left right tap; do
        key="$edge.$direction"
        printf '  %-6s → %s\n' "$direction" "$(action_name "${bindings[$key]:-0}")" >&2
    done
    if [ "$input_source" = none ]; then continue; fi
    while :; do
        ask '1) Keep suggested  2) Customize  3) Clear edge  [Enter = 1]: '
        case "$answer" in
            ''|1) break ;;
            2)
                show_actions
                for direction in up down left right tap; do
                    key="$edge.$direction"
                    current="${bindings[$key]:-0}"
                    while :; do
                        ask "$edge / $direction [$(action_name "$current"); Enter keeps]: "
                        if [ -z "$answer" ]; then break; fi
                        if [[ "$answer" =~ ^([0-9]|1[0-5])$ ]] && {
                            [ "$answer" -le 11 ] || [ -n "$workspace_prev" ];
                        }; then
                            if [ "$answer" -eq 0 ]; then unset 'bindings[$key]';
                            else bindings[$key]="$answer"; fi
                            break
                        fi
                        echo 'Choose a number from the action menu.' >&2
                    done
                done
                break ;;
            3)
                for direction in up down left right tap; do
                    key="$edge.$direction"
                    unset 'bindings[$key]'
                done
                break ;;
            *) echo 'Choose 1, 2, or 3.' >&2 ;;
        esac
    done
done

printf '\n%bYOUR EDGE LOADOUT%b\n' "$ui_accent" "$ui_reset" >&2
for edge in left right top bottom; do
    printf '  %-6s ' "${edge^^}" >&2
    for direction in up down left right tap; do
        key="$edge.$direction"
        if [ -n "${bindings[$key]:-}" ]; then
            printf '%s=%s  ' "$direction" "$(action_name "${bindings[$key]}")" >&2
        fi
    done
    printf '\n' >&2
done

emit_bindings() {
    local format="$1" edge direction key id cmd
    for edge in left right top bottom; do
        for direction in up down left right tap; do
            key="$edge.$direction"
            id="${bindings[$key]:-0}"
            [ "$id" -ne 0 ] || continue
            cmd="$(action_command "$id")"
            if [ "$format" = nix ]; then
                printf '    %s.%s = { action = "command"; cmd = "%s"; };\n' "$edge" "$direction" "$cmd"
            else
                printf '\n[gestures.%s.%s]\naction = "command"\ncmd = "%s"\n' "$edge" "$direction" "$cmd"
            fi
        done
    done
}

if [ "$declarative" -eq 1 ]; then
    printf '\n%bYOUR NIXOS LOADOUT%b\n' "$ui_accent" "$ui_reset" >&2
    cat <<'NIX'
# Import Bezel's Home Manager module, then add:
services.bezel.enable = true;
home.packages = with pkgs; [ wireplumber brightnessctl playerctl ];
services.bezel.config = {
  zones = { left_width = 0.08; right_width = 0.08; top_height = 0.08; bottom_height = 0.08; };
  gestures = {
NIX
    emit_bindings nix
    cat <<'NIX'
  };
};
NIX
    printf 'Also enable hardware.uinput and add your user to the input and uinput groups in NixOS.\n' >&2
    exit 0
fi

mkdir -p "$config_dir"
generated="$(mktemp "$config_dir/.config.toml.XXXXXX")"
{
    printf '# Generated by Bezel onboarding for %s.\n' "$desktop"
    printf '[zones]\nleft_width = 0.08\nright_width = 0.08\ntop_height = 0.08\nbottom_height = 0.08\n'
    emit_bindings toml
} > "$generated"

if [ -e "$config_file" ] || [ -L "$config_file" ]; then
    printf '\nYour current config is safe at %s.\n' "$config_file" >&2
    printf 'Replacing it also replaces custom device, zone, and OSD settings; a backup is made first.\n' >&2
    if [ "$input_source" = none ]; then answer=1
    else
        while :; do
            ask '1) Save preview  2) Back up and replace current config  3) Cancel  [Enter = 1]: '
            case "$answer" in ''|1|2|3) break ;; *) echo 'Choose 1, 2, or 3.' >&2 ;; esac
        done
    fi
    case "$answer" in
        ''|1)
            preview="$(mktemp "$config_dir/config.preview.XXXXXX.toml")"
            mv "$generated" "$preview"
            echo "Preview saved to $preview (current config untouched)" ;;
        2)
            backup="$(mktemp "$config_file.backup.XXXXXX")"
            cp -p "$config_file" "$backup"
            mv "$generated" "$config_file"
            echo "Saved old config to $backup"
            echo "New config installed at $config_file. Restart Bezel to load it." ;;
        3) rm -- "$generated"; echo 'Cancelled; current config untouched.' ;;
    esac
else
    mv "$generated" "$config_file"
    echo "Created $config_file for $desktop"
fi
