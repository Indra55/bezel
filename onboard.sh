#!/usr/bin/env bash
# Interactive first-run configuration. The installer uses --if-missing on upgrades.
set -euo pipefail

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/bezel"
config_file="$config_dir/config.toml"
desktop_explicit=0
if_missing=0
declarative=0
color_mode=auto
plain_mode=0
animation=1
ui_frontend=0
ui_active=0
ui_saved_stty=
ui_fd=0
generated=
editing_existing=0

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
        --plain) plain_mode=1; shift ;;
        --no-animation) animation=0; shift ;;
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
    ui_soft=$'\033[38;2;183;175;166m'
    ui_halo=$'\033[38;2;98;75;59m'
    ui_faint=$'\033[38;2;65;55;47m'
    ui_reset=$'\033[0m'
fi

ask() {
    local prompt="$1"
    if [ "$ui_frontend" -eq 1 ]; then panel_input "$prompt" "${ui_input_initial:-}"; return; fi
    printf '%s' "$prompt" >&2
    case "$input_source" in
        stdin) IFS= read -r answer || answer="" ;;
        tty) IFS= read -r answer </dev/tty || answer="" ;;
        none) answer=""; printf '\n' >&2 ;;
    esac
}

show_welcome() {
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
printf 'A small control surface. Made yours.\n' >&2

}

desktop_name() {
    case "$1" in
        hyprland) echo Hyprland ;; niri) echo Niri ;; sway) echo Sway ;;
        plasma) echo 'KDE Plasma' ;; gnome) echo GNOME ;; generic) echo 'Other Wayland desktop' ;;
    esac
}


declare -A extra_commands=()

pick_command() {
    local candidate
    for candidate in "$@"; do
        if command -v "${candidate%% *}" >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return
        fi
    done
}

configure_desktop() {
workspace_prev=""; workspace_next=""; move_prev=""; move_next=""; toggle_magic=""
extra_commands=()
local direction id=20 launcher picker
case "$desktop" in
    hyprland)
        workspace_prev="hyprctl dispatch 'hl.dsp.focus({ workspace = \"e-1\" })'"
        workspace_next="hyprctl dispatch 'hl.dsp.focus({ workspace = \"e+1\" })'"
        move_prev="hyprctl dispatch 'hl.dsp.window.move({ workspace = \"e-1\", follow = true })'"
        move_next="hyprctl dispatch 'hl.dsp.window.move({ workspace = \"e+1\", follow = true })'"
        toggle_magic="hyprctl dispatch 'hl.dsp.workspace.toggle_special(\"magic\")'"
        extra_commands[17]="hyprctl dispatch 'hl.dsp.window.close()'"
        extra_commands[18]="hyprctl dispatch 'hl.dsp.window.fullscreen({ mode = \"fullscreen\", action = \"toggle\" })'"
        extra_commands[19]="hyprctl dispatch 'hl.dsp.window.float({ action = \"toggle\" })'"
        for direction in left right up down; do
            extra_commands[$id]="hyprctl dispatch 'hl.dsp.focus({ direction = \"$direction\" })'"
            id=$((id+1))
        done ;;
    niri)
        workspace_prev='niri msg action focus-workspace-up'
        workspace_next='niri msg action focus-workspace-down'
        move_prev='niri msg action move-window-to-workspace-up'
        move_next='niri msg action move-window-to-workspace-down'
        extra_commands[17]='niri msg action close-window'
        extra_commands[18]='niri msg action fullscreen-window'
        extra_commands[19]='niri msg action toggle-window-floating'
        extra_commands[20]='niri msg action focus-column-left'
        extra_commands[21]='niri msg action focus-column-right'
        extra_commands[22]='niri msg action focus-window-up'
        extra_commands[23]='niri msg action focus-window-down' ;;
    sway)
        workspace_prev='swaymsg workspace prev'
        workspace_next='swaymsg workspace next'
        move_prev='swaymsg move container to workspace prev'
        move_next='swaymsg move container to workspace next'
        extra_commands[17]='swaymsg kill'
        extra_commands[18]='swaymsg fullscreen toggle'
        extra_commands[19]='swaymsg floating toggle'
        for direction in left right up down; do
            extra_commands[$id]="swaymsg focus $direction"
            id=$((id+1))
        done ;;
esac
launcher="$(pick_command fuzzel 'wofi --show drun' 'rofi -show drun')"
extra_commands[24]="$launcher"
extra_commands[25]="$(pick_command foot kitty alacritty 'wezterm start' konsole gnome-terminal xterm)"
case "$desktop" in
    plasma|gnome) extra_commands[26]="$(pick_command 'loginctl lock-session')" ;;
    *) extra_commands[26]="$(pick_command hyprlock swaylock 'loginctl lock-session')" ;;
esac
if [ "$desktop" = niri ]; then
    extra_commands[27]='niri msg action screenshot'
    extra_commands[28]='niri msg action screenshot-screen'
elif [[ "$desktop" = hyprland || "$desktop" = sway || "$desktop" = generic ]] &&
    command -v grim >/dev/null 2>&1 && command -v wl-copy >/dev/null 2>&1; then
    extra_commands[28]='grim - | wl-copy --type image/png'
    if command -v slurp >/dev/null 2>&1; then
        # Expanded when the gesture runs.
        # shellcheck disable=SC2016
        extra_commands[27]='region=$(slurp) && [ -n "$region" ] && grim -g "$region" - | wl-copy --type image/png'
    fi
fi
picker="$(pick_command 'fuzzel --dmenu' 'wofi --dmenu' 'rofi -dmenu')"
if [ -n "$picker" ] && command -v cliphist >/dev/null 2>&1 && command -v wl-copy >/dev/null 2>&1; then
    extra_commands[29]="entry=\$(cliphist list | $picker) && [ -n \"\$entry\" ] && printf '%s\\n' \"\$entry\" | cliphist decode | wl-copy"
fi
extra_commands[30]="$(pick_command 'swaync-client --toggle-dnd' 'dunstctl set-paused toggle')"
}
configure_desktop

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
if [ -n "$toggle_magic" ]; then bindings[top.tap]=16; fi

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
        16) echo 'Toggle magic workspace' ;;
        17) echo 'Close focused window' ;; 18) echo 'Toggle fullscreen' ;;
        19) echo 'Toggle floating' ;; 20) echo 'Focus window left' ;;
        21) echo 'Focus window right' ;; 22) echo 'Focus window up' ;;
        23) echo 'Focus window down' ;; 24) echo 'Open application launcher' ;;
        25) echo 'Open terminal' ;; 26) echo 'Lock screen' ;;
        27) echo 'Screenshot selected region' ;; 28) echo 'Screenshot entire screen' ;;
        29) echo 'Open clipboard history' ;; 30) echo 'Toggle Do Not Disturb' ;;
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
        16) echo "$toggle_magic" ;;
        17|18|19|2[0-9]|30) echo "${extra_commands[$1]:-}" ;;
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
    for id in {1..30}; do
        if [ -n "$(action_command "$id")" ]; then
            printf ' %2d) %s\n' "$id" "$(action_name "$id")" >&2
        fi
    done
}

# Advanced bindings override the quick setup's single-finger entries.
declare -A advanced_commands=() advanced_tuning=()
declare -A recognition=( [join_ms]=80 [swipe_distance]=0.05 [tap_distance]=0.02 [tap_ms]=200 [double_tap_ms]=300 [hold_ms]=500 [step_distance]=0.03 )
recognition_keys=(join_ms swipe_distance tap_distance tap_ms double_tap_ms hold_ms step_distance)
gesture_names=(tap double_tap hold swipe_up swipe_down swipe_left swipe_right swipe_up_left swipe_up_right swipe_down_left swipe_down_right slide_up slide_down slide_left slide_right)

# Python's TOML parser handles quoted keys, escapes, and multiline commands.
existing_config() {
    python3 - "$config_file" "$@" <<'PYTHON'
import datetime
import json
from pathlib import Path
import sys
try:
    import tomllib
except ImportError:
    print("Editing an existing config requires Python 3.11 or newer.", file=sys.stderr)
    sys.exit(1)

try:
    source = tomllib.loads(Path(sys.argv[1]).read_text())
    if len(sys.argv) == 2:
        edges = {"left", "right", "top", "bottom"}
        supported_gestures = {"tap", "double_tap", "hold", "swipe_up", "swipe_down",
                    "swipe_left", "swipe_right", "swipe_up_left", "swipe_up_right",
                    "swipe_down_left", "swipe_down_right", "slide_up", "slide_down",
                    "slide_left", "slide_right"}
        def record(kind, key, value):
            value = str(value)
            if "\0" in value:
                raise ValueError("NUL characters cannot be edited")
            sys.stdout.buffer.write((kind + "\0" + key + "\0" + value + "\0").encode())

        for edge, gestures in source.get("gestures", {}).items():
            if edge not in edges:
                raise ValueError(f"Unknown edge: {edge}")
            for direction, action in gestures.items():
                if direction not in {"up", "down", "left", "right", "tap"} or action.get("action") != "command":
                    raise ValueError("Unsupported legacy gesture")
                gesture = "tap" if direction == "tap" else "swipe_" + direction
                record("command", f"{edge}.1.{gesture}", action["cmd"])
        for binding in source.get("bindings", []):
            if (binding["zone"] not in edges or binding["fingers"] not in range(1, 5)
                    or binding["gesture"] not in supported_gestures or binding.get("action") != "command"):
                raise ValueError("Unsupported binding")
            key = f"{binding['zone']}.{binding['fingers']}.{binding['gesture']}"
            record("command", key, binding["cmd"])
            for setting in ("join_ms", "swipe_distance", "tap_distance", "tap_ms",
                            "double_tap_ms", "hold_ms", "step_distance"):
                if setting in binding:
                    record("tuning", key + "." + setting, binding[setting])
        for key, value in source.get("recognition", {}).items():
            record("recognition", key, value)
    else:
        target = Path(sys.argv[2])
        updated = tomllib.loads(target.read_text())
        for key in ("gestures", "bindings", "recognition"):
            source.pop(key, None)
            if key in updated:
                source[key] = updated[key]

        def scalar(value):
            if isinstance(value, (datetime.date, datetime.time)):
                return value.isoformat()
            if isinstance(value, list):
                return "[" + ", ".join(scalar(v) for v in value) + "]"
            if isinstance(value, dict):
                return "{ " + ", ".join(json.dumps(k) + " = " + scalar(v) for k, v in value.items()) + " }"
            if isinstance(value, float):
                return str(value)
            return json.dumps(value, ensure_ascii=False)

        lines = []
        def table(values, path=()):
            for key, value in values.items():
                if not isinstance(value, dict):
                    lines.append(json.dumps(key) + " = " + scalar(value))
            for key, value in values.items():
                if isinstance(value, dict):
                    child = path + (key,)
                    lines.append("\n[" + ".".join(json.dumps(k) for k in child) + "]")
                    table(value, child)
        table(source)
        text = "\n".join(lines) + "\n"
        tomllib.loads(text)
        target.write_text(text)
except (OSError, ValueError, KeyError, TypeError) as error:
    print(f"Cannot edit existing config: {error}", file=sys.stderr)
    sys.exit(1)
PYTHON
}

load_existing() {
    [ "$declarative" -eq 0 ] && [ -f "$config_file" ] || return 0
    local loaded kind key value
    loaded="$(mktemp)"
    if ! existing_config > "$loaded"; then rm -f -- "$loaded"; exit 1; fi
    bindings=()
    while IFS= read -r -d '' kind && IFS= read -r -d '' key && IFS= read -r -d '' value; do
        case "$kind" in
            command) advanced_commands[$key]="$value" ;;
            tuning) advanced_tuning[$key]="$value" ;;
            recognition) recognition[$key]="$value" ;;
        esac
    done < "$loaded"
    rm -f -- "$loaded"
    editing_existing=1
}

choose() {
    local prompt="$1"; shift
    local choices=("$@") i
    for i in "${!choices[@]}"; do printf ' %2d) %s\n' "$((i + 1))" "${choices[$i]}" >&2; done
    while :; do
        ask "$prompt [Enter = 1]: "
        answer="${answer:-1}"
        if [[ "$answer" =~ ^[1-9][0-9]?$ ]] && (( answer <= ${#choices[@]} )); then
            answer="${choices[$((answer - 1))]}"; return
        fi
        echo 'Choose a number from the menu.' >&2
    done
}

read_setting() {
    local name="$1" current="$2" optional="$3"
    while :; do
        ask "$name [$current; Enter keeps${optional:+; d = default}]: " || return 1
        [ -n "$answer" ] || return 0
        if [ -n "$optional" ] && [ "$answer" = d ]; then return 0; fi
        if [[ "$name" = *_ms ]]; then
            if [[ "$answer" =~ ^[1-9][0-9]{0,4}$ ]] && (( answer <= 10000 )); then
                if [ "$name" = join_ms ]; then
                    if (( answer > 1000 || answer > recognition[tap_ms] || answer > recognition[hold_ms] )); then
                        setting_error 'Join must be <= 1000 ms and <= tap/hold times.'; continue
                    fi
                    local setting conflict=0
                    for setting in "${!advanced_tuning[@]}"; do
                        if [[ "$setting" = *.tap_ms || "$setting" = *.hold_ms ]] && (( answer > advanced_tuning[$setting] )); then conflict=1; fi
                    done
                    if [ "$conflict" -eq 1 ]; then setting_error 'Join time exceeds a binding tap/hold override.'; continue; fi
                elif [[ "$name" = tap_ms || "$name" = hold_ms ]] && (( answer < recognition[join_ms] )); then
                    setting_error 'Time must be at least join_ms.'; continue
                fi
                return
            fi
        elif [[ "$answer" =~ ^(0\.[0-9]+|1(\.0+)?)$ ]] && awk -v value="$answer" 'BEGIN { exit !(value >= 0.001 && value <= 1) }'; then return
        fi
        setting_error 'Use 1-10000 ms, or distance 0.001-1.'
    done
}

remove_advanced_binding() {
    unset 'advanced_commands[$key]'
    for setting in "${recognition_keys[@]}"; do unset 'advanced_tuning[$key.$setting]'; done
    if [ "$fingers" = 1 ]; then
        case "$gesture" in
            tap|swipe_up|swipe_down|swipe_left|swipe_right) unset 'bindings[$advanced_edge.${gesture#swipe_}]' ;;
        esac
    fi
}


escape_command() {
    local value="$1" format="$2"
    value="${value//\\/\\\\}"; value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"; value="${value//$'\b'/\\b}"; value="${value//$'\f'/\\f}"
    value="${value//$'\t'/\\t}"; value="${value//$'\r'/\\r}"
    if [ "$format" = nix ]; then value="${value//\$\{/\\\$\{}"; fi
    printf '%s' "$value"
}

emit_advanced() {
    local format="$1" key edge fingers gesture setting cmd
    if [ "$format" = nix ]; then printf '  recognition = {\n'; else printf '\n[recognition]\n'; fi
    for setting in "${recognition_keys[@]}"; do
        if [ "$format" = nix ]; then printf '    %s = %s;\n' "$setting" "${recognition[$setting]}"
        else printf '%s = %s\n' "$setting" "${recognition[$setting]}"; fi
    done
    if [ "$format" = nix ]; then printf '  };\n  bindings = [\n'; fi
    for key in "${!advanced_commands[@]}"; do
        IFS=. read -r edge fingers gesture <<< "$key"
        cmd="$(escape_command "${advanced_commands[$key]}" "$format")"
        if [ "$format" = nix ]; then
            printf '    { zone = "%s"; fingers = %s; gesture = "%s"; action = "command"; cmd = "%s";\n' "$edge" "$fingers" "$gesture" "$cmd"
        else
            printf '\n[[bindings]]\nzone = "%s"\nfingers = %s\ngesture = "%s"\naction = "command"\ncmd = "%s"\n' "$edge" "$fingers" "$gesture" "$cmd"
        fi
        for setting in "${recognition_keys[@]}"; do
            if [ -n "${advanced_tuning[$key.$setting]:-}" ]; then
                if [ "$format" = nix ]; then printf '      %s = %s;\n' "$setting" "${advanced_tuning[$key.$setting]}"
                else printf '%s = %s\n' "$setting" "${advanced_tuning[$key.$setting]}"; fi
            fi
        done
        if [ "$format" = nix ]; then printf '    }\n'; fi
    done
    if [ "$format" = nix ]; then printf '  ];\n'; fi
}


emit_bindings() {
    local format="$1" edge direction key id cmd
    for edge in left right top bottom; do
        for direction in up down left right tap; do
            key="$edge.$direction"
            id="${bindings[$key]:-0}"
            [ "$id" -ne 0 ] || continue
            cmd="$(action_command "$id")"
            cmd="${cmd//\\/\\\\}"
            cmd="${cmd//\"/\\\"}"
            if [ "$format" = nix ]; then
                printf '    %s.%s = { action = "command"; cmd = "%s"; };\n' "$edge" "$direction" "$cmd"
            else
                printf '\n[gestures.%s.%s]\naction = "command"\ncmd = "%s"\n' "$edge" "$direction" "$cmd"
            fi
        done
    done
}

setting_error() {
    if [ "$ui_frontend" -eq 1 ]; then ui_notice="$1"; else printf '%s\n' "$1" >&2; fi
}

plain_wizard() {
    show_welcome
if [ "$desktop_explicit" -eq 0 ] && [ "$editing_existing" -eq 0 ]; then
    printf '\nSession detected: %s\n' "$(desktop_name "$desktop")" >&2
    printf '  1) Hyprland   2) Niri   3) Sway\n' >&2
    printf '  4) KDE Plasma 5) GNOME  6) Other Wayland\n' >&2
    printf '  7) NixOS\n' >&2
    while :; do
        ask "Pick your desktop [Enter = $(desktop_name "$desktop")]: "
        case "$answer" in
            '') break ;; 1) desktop=hyprland; break ;; 2) desktop=niri; break ;;
            3) desktop=sway; break ;; 4) desktop=plasma; break ;;
            5) desktop=gnome; break ;; 6) desktop=generic; break ;;
            7)
                declarative=1
                printf 'NixOS selected. Now choose your desktop (1-6), or press Enter for %s.\n' "$(desktop_name "$desktop")" >&2
                ;;
            *) echo 'Choose 1-7, or press Enter.' >&2 ;;
        esac
    done
fi

    configure_desktop
    set_desktop_defaults
step=0
if [ "$editing_existing" -eq 0 ]; then
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
                        if [[ "$answer" =~ ^([0-9]|[12][0-9]|30)$ ]] && {
                            [ "$answer" = 0 ] || [ -n "$(action_command "$answer")" ];
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
fi

if [ "$input_source" != none ]; then
    while :; do
        printf '\nAdvanced bindings (override quick setup):\n' >&2
        for key in "${!advanced_commands[@]}"; do printf '  %s → %s\n' "$key" "${advanced_commands[$key]}" >&2; done
        ask '1) Add/edit binding  2) Delete binding  3) Shared sensitivity  4) Finish [Enter = 4]: '
        case "${answer:-4}" in
            1|2)
                operation="$answer"
                choose 'Edge' left right top bottom; advanced_edge="$answer"
                choose 'Fingers' 1 2 3 4; fingers="$answer"
                choose 'Gesture' "${gesture_names[@]}"; gesture="$answer"
                key="$advanced_edge.$fingers.$gesture"
                if [ "$operation" = 2 ]; then
                    remove_advanced_binding
                    continue
                fi
                show_actions
                echo ' 31) Custom shell command' >&2
                while :; do
                    ask 'Action [Enter keeps existing, or leaves unused]: '
                    if [ -z "$answer" ]; then break; fi
                    if [ "$answer" = 31 ]; then
                        ask 'Shell command: '
                        if [[ "$answer" =~ [^[:space:]] ]]; then advanced_commands[$key]="$answer"; break; fi
                    elif [ "$answer" = 0 ]; then remove_advanced_binding; break
                    elif [[ "$answer" =~ ^([1-9]|[12][0-9]|30)$ ]]; then
                        cmd="$(action_command "$answer")"
                        if [ -n "$cmd" ]; then advanced_commands[$key]="$cmd"; break; fi
                    fi
                    echo 'Choose an available action or enter a nonempty custom command.' >&2
                done
                [ -n "${advanced_commands[$key]:-}" ] || continue
                case "$gesture" in
                    tap) tuning_keys=(tap_distance tap_ms) ;;
                    double_tap) tuning_keys=(tap_distance tap_ms double_tap_ms) ;;
                    hold) tuning_keys=(tap_distance hold_ms) ;;
                    slide_*) tuning_keys=(step_distance) ;;
                    swipe_*) tuning_keys=(swipe_distance) ;;
                esac
                for setting in "${tuning_keys[@]}"; do
                    read_setting "$setting" "${advanced_tuning[$key.$setting]:-${recognition[$setting]}}" optional
                    case "$answer" in '') ;; d) unset 'advanced_tuning[$key.$setting]' ;; *) advanced_tuning[$key.$setting]="$answer" ;; esac
                done ;;
            3)
                for setting in "${recognition_keys[@]}"; do
                    read_setting "$setting" "${recognition[$setting]}" ''
                    if [ -n "$answer" ]; then recognition[$setting]="$answer"; fi
                done ;;
            4|'') break ;;
            *) echo 'Choose 1, 2, 3, or 4.' >&2 ;;
        esac
    done
fi
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
    for key in "${!advanced_commands[@]}"; do
        printf '  %s → %s\n' "$key" "${advanced_commands[$key]}" >&2
    done
}

set_desktop_defaults() {
    [ "$editing_existing" -eq 0 ] || return 0
    if [ -n "$workspace_prev" ]; then bindings[top.left]=12; bindings[top.right]=13; fi
    if [ -n "$toggle_magic" ]; then bindings[top.tap]=16; fi
}

# The panel uses stderr for drawing and a dedicated terminal fd for input.
# stdout remains available for declarative configuration output.
ui_resized=1 ui_rows=24 ui_cols=80 ui_top=1 ui_left=1
ui_edge=left ui_fingers=1 ui_gesture=swipe_up ui_action='' ui_notice=''
ui_stage=1 ui_frame=0 ui_selected=0 ui_kind='' ui_title='' ui_digits=''
ui_input_initial='' ui_editing=0 ui_text='' ui_cursor=0
ui_preview=none ui_feedback='' ui_pending='' ui_desktop_preview=generic
menu_labels=() menu_values=() ui_previous=() ui_lines=()

panel_cleanup() {
    if [ -n "$ui_saved_stty" ]; then stty "$ui_saved_stty" <&"$ui_fd" 2>/dev/null || true; fi
    if [ "$ui_active" -eq 1 ]; then printf '\033[0m\033[?25h\033[?1049l' >&2; fi
    ui_active=0
    if [ -n "$generated" ] && [ -f "$generated" ]; then rm -f -- "$generated"; fi
}

panel_dimensions() {
    local size
    size="$(stty size <&"$ui_fd" 2>/dev/null)" || size='0 0'
    read -r ui_rows ui_cols <<< "$size"
    ui_rows=${ui_rows:-0}; ui_cols=${ui_cols:-0}
    ui_top=$(( (ui_rows - 22) / 2 + 1 )); ui_left=$(( (ui_cols - 76) / 2 + 1 ))
    ui_resized=0
}

panel_init() {
    [ "$plain_mode" -eq 0 ] && [ -t 2 ] && [ "$input_source" != none ] || return 1
    case "${TERM:-dumb}" in dumb|unknown|'') return 1 ;; esac
    case "${LC_ALL:-${LC_CTYPE:-${LANG:-}}}" in *UTF-8*|*utf8*|*UTF8*|*utf-8*) ;; *) return 1 ;; esac
    command -v stty >/dev/null || return 1
    if [ "$input_source" = stdin ]; then exec {ui_fd}<&0; else exec {ui_fd}</dev/tty; fi
    panel_dimensions
    (( ui_rows >= 24 && ui_cols >= 80 )) || return 1
    ui_saved_stty="$(stty -g <&"$ui_fd")" || return 1
    stty -echo -icanon min 1 time 0 <&"$ui_fd" || return 1
    ui_frontend=1 ui_active=1
    trap panel_cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    trap 'exit 129' HUP
    trap 'ui_resized=1' WINCH
    printf '\033[?1049h\033[?25l\033[2J' >&2
}

panel_resize() {
    [ "$ui_resized" -eq 1 ] || return 0
    panel_dimensions
    if (( ui_rows < 24 || ui_cols < 80 )); then
        panel_cleanup
        ui_saved_stty=''
        ui_notice='Terminal is small; continuing with numbered prompts.'
        printf '\n%s\n' "$ui_notice" >&2
    elif [ "$ui_active" -eq 1 ]; then
        printf '\033[2J' >&2
        ui_previous=()
    fi
}

# Render untrusted labels as text; command contents are never interpreted.
panel_safe() {
    safe_text="${1//[[:cntrl:]]/ }"
}

panel_fit() {
    local text="$1" limit="$2" ch code width i
    fitted='' fitted_width=0
    for ((i=0;i<${#text};i++)); do
        ch="${text:i:1}"; width=1
        printf -v code '%d' "'$ch"
        if (( (code>=0x300 && code<=0x36f) || (code>=0xfe00 && code<=0xfe0f) )); then width=0
        elif (( (code>=0x1100 && code<=0x115f) || (code>=0x2e80 && code<=0xa4cf) ||
            (code>=0xac00 && code<=0xd7a3) || (code>=0xf900 && code<=0xfaff) ||
            (code>=0xff01 && code<=0xff60) || (code>=0x1f300 && code<=0x1faff) )); then width=2; fi
        ((fitted_width+width<=limit)) || break
        fitted+="$ch"; fitted_width=$((fitted_width+width))
    done
}

panel_line() {
    local row="$1" text="$2" style="${3:-$ui_soft}"
    panel_safe "$text"; panel_fit "$safe_text" 72
    printf -v "ui_lines[$row]" '%s│%s %s%*s %s│%s' "$ui_faint" "$style" "$fitted" "$((72-fitted_width))" '' "$ui_faint" "$ui_reset"
}

panel_columns() {
    local row="$1" left="$2" right="$3" selected="${4:-0}" style="$ui_soft" left_width
    panel_fit "$left" 33; left="$fitted" left_width="$fitted_width"
    panel_safe "$right"; panel_fit "$safe_text" 36
    [ "$selected" -eq 0 ] || style="$ui_accent"
    printf -v "ui_lines[$row]" '%s│%s %s%*s %s│%s %s%*s %s│%s' "$ui_faint" "$ui_soft" "$left" "$((33-left_width))" '' "$ui_faint" "$style" "$fitted" "$((36-fitted_width))" '' "$ui_faint" "$ui_reset"
}

panel_context() {
    local value="${menu_values[$ui_selected]}"
    ui_preview=none
    case "$ui_kind" in
        desktop) ui_desktop_preview="$value" ;;
        edge) ui_edge="$value"; ui_preview=edge ;;
        edge_controls) ui_preview=edge ;;
        fingers) ui_fingers="$value"; ui_preview=fingers ;;
        gesture) if [ "$value" != "done" ]; then ui_gesture="$value"; ui_preview=gesture; else ui_preview=edge; fi ;;
        action) ui_action="${menu_labels[$ui_selected]}"; ui_preview=pose ;;
        direction)
            if [ "$value" != "done" ]; then
                ui_preview=gesture
                ui_gesture="swipe_$value"; [ "$value" != tap ] || ui_gesture=tap
                ui_action="$(action_name "${bindings[$ui_edge.$value]:-0}")"
            fi ;;
        review)
            ui_notice=''
            if [[ "$value" = binding:* ]]; then
                ui_preview=gesture
                IFS=. read -r ui_edge ui_fingers ui_gesture <<< "${value#binding:}"
                ui_action="${advanced_commands[${value#binding:}]}"
            elif [[ "$value" = legacy:* ]]; then
                ui_preview=gesture
                local edge direction
                IFS=. read -r edge direction <<< "${value#legacy:}"
                ui_edge="$edge" ui_fingers=1 ui_gesture="swipe_$direction"
                [ "$direction" != tap ] || ui_gesture=tap
                ui_action="$(action_name "${bindings[$edge.$direction]}")"
            elif [ "$value" = 1 ] && [ "$declarative" -eq 0 ]; then
                ui_notice='Saves controls with a backup; keeps device, zone and OSD settings.'
            fi ;;
    esac
}

panel_trackpad() {
    local row col f x y dx=0 dy=0 phase="$ui_frame" mark='●' trail='·' offset
    local -a cells=()
    (( phase <= 8 )) || phase=8
    case "$ui_gesture" in
        *up*) dy=$((-phase / 2)) ;; *down*) dy=$((phase / 2)) ;;
    esac
    case "$ui_gesture" in
        *left*) dx=$((-phase)) ;; *right*) dx=$phase ;;
    esac
    case "$ui_gesture" in
        tap) if (( phase < 2 || phase > 5 )); then mark='○'; fi ;;
        double_tap) if (( phase == 0 || phase == 3 || phase == 4 || phase >= 7 )); then mark='○'; fi ;;
        hold) if (( phase < 8 )); then mark='○'; fi ;;
    esac
    local count="$ui_fingers" min_x max_x active_edge="$ui_edge"
    case "$ui_preview" in
        none) count=0; active_edge='' ;;
        edge) count=0 ;;
        fingers|pose) dx=0; dy=0; mark='○' ;;
    esac
    case "$ui_edge" in
        left) min_x=2; max_x=$((2+(ui_fingers-1)*6)) ;;
        right) min_x=$((25-(ui_fingers-1)*6)); max_x=25 ;;
        *) min_x=4; max_x=$((4+(ui_fingers-1)*6)) ;;
    esac
    ((min_x+dx>=1)) || dx=$((1-min_x))
    ((max_x+dx<=26)) || dx=$((26-max_x))
    for ((f=0; f<count; f++)); do
        case "$ui_edge" in
            left) x=$((2+f*6)); y=3 ;;
            right) x=$((25-f*6)); y=3 ;;
            top) x=$((4+f*6)); y=1 ;;
            bottom) x=$((4+f*6)); y=5 ;;
        esac
        case "$ui_gesture" in
            *up*) if [[ "$ui_edge" = left || "$ui_edge" = right ]]; then y=5; fi ;;
            *down*) if [[ "$ui_edge" = left || "$ui_edge" = right ]]; then y=1; fi ;;
        esac
        x=$((x+dx)); y=$((y+dy)); ((x>=1)) || x=1; ((x<=26)) || x=26
        ((y>=0)) || y=0; ((y<=6)) || y=6
        if [ "$ui_preview" = gesture ] && [[ "$ui_gesture" = swipe_* || "$ui_gesture" = slide_* ]] && ((phase>0 && phase<8)); then
            offset=$((y*28+x))
            if ((dx>0 && x>1)); then cells[offset-1]="$trail"
            elif ((dx<0 && x<26)); then cells[offset+1]="$trail"
            elif ((dy>0 && y>0)); then cells[offset-28]="$trail"
            elif ((dy<0 && y<6)); then cells[offset+28]="$trail"; fi
        fi
        cells[y*28+x]="$mark"
    done
    pad_lines=('  ╭────────────────────────────╮')
    if [ "$active_edge" = top ]; then pad_lines[0]='  ╭━━━━━━━━━━━━━━━━━━━━━━━━━━━━╮'; fi
    for ((row=0; row<7; row++)); do
        local line='  │'
        [ "$active_edge" != left ] || line='  ┃'
        for ((col=0; col<28; col++)); do line+="${cells[$((row*28+col))]:- }"; done
        if [ "$active_edge" = right ]; then line+='┃'; else line+='│'; fi
        pad_lines+=("$line")
    done
    if [ "$active_edge" = bottom ]; then pad_lines+=('  ╰━━━━━━━━━━━━━━━━━━━━━━━━━━━━╯')
    else pad_lines+=('  ╰────────────────────────────╯'); fi
}

# Terminal silhouettes of desktop marks; embedded so setup also works offline.
# References: hypr.land, github.com/niri-wm/artwork, swaywm.org,
# kde.org/stuff/clipart.php, brand.gnome.org.
# Nix snowflake: github.com/NixOS/nixos-artwork/tree/master/logo (CC-BY).
# Simon Frankau / Tim Cuthbertson; adapted to monochrome terminal characters.
panel_desktop_logo() {
    case "$ui_desktop_preview" in
        hyprland) mapfile -t pad_lines <<'LOGO'
              ▗  ▖
             ▟▛  ▜▙▁
           ▗█▘    ▝█▖
          ▟▛        ▜▙
         ▐▛          ▜▌
         █▌          ▐▊
         ▐▙          ▟▌
          ▀▙▁      ▁▟▀
           ▔▀▜▅▅▅▆▛▀▔
LOGO
            ;;
        niri) mapfile -t pad_lines <<'LOGO'
           ▗▇▇▅▃
           ▐█████▄
           ▐███████▖
           ▐██▘▔▀▜██▖
           ▐█▊    ▜█▋
           ▝█▉    ▟█▘
            ▔▀▘  ▝▀
             ▄▆▆▆▆▄
             ▜████▛
LOGO
            ;;
        sway) mapfile -t pad_lines <<'LOGO'
              ▂▃▄▅▁▂▃▃▃▂
             ▀▀▔▗▇█████▂
           ▄ ▅▛▘▗▔▔▀▀▀▔▔
             ▂▖▝▔▗▆▀▀▂▃
          ▁▖▀▔▀ ▔  ▝▀▀▔
         ▗▚▃▄▄▃▄▅▆▆▅▄▂▁
       ▃▊▝▐███████████▛▀▃
        ▔▀▀▗▟▛▀██▀▜▙▖▀▀▔
             ▔▀▀▀▀▔
LOGO
            ;;
        plasma) mapfile -t pad_lines <<'LOGO'
              ▐▇█▍  ▗▇▆▅
          ▃▂  ▐██▍ ▟██▛▔
         ▝██▇▊▐██▙███▘
          ▐▛▔ ▐██▛██▙▖
       ▗▇▇█▏  ▐██▍▝███▄
       ▔▀▀█▖  ▐██▍  ▜██▙
          ▟█▃▁▝▔▔ ▂▟▙▔▔
         ▝█▛▀▀▇▆▆▛▀▀██
              ▐█▉
LOGO
            ;;
        gnome) mapfile -t pad_lines <<'LOGO'
                   ▁▂▁
            ▂▖▕▇▊ ▟███
          ▃ ▜▊ ▀▘ ██▀
          ▀▌ ▂▄▅▆▆▅▖
           ▗███████▛
           ██████▀▔
           ▝███▘ ▗▆▆▏
            ▝▜█▇▆██▀
               ▔▔▔
LOGO
            ;;
        nixos) mapfile -t pad_lines <<'LOGO'
           ▗▖  ▗▃  ▂▂
           ▝█▖  ▜█▄█▘
         ▟▇▇██▇▇▖▜█▌  ▄
           ▗▅     ▝▛▗█▛
       ▐▇▇▇█▘      ▗██▇▇▋
         ▟█▘▟▖     ▀▘
         ▀  ▐█▙▝▇▇▇▇▇▇▌
           ▗█▀█▙  ▝█▖
           ▔▔  ▀▘  ▔▔
LOGO
            ;;
        *) pad_lines=(
            ''
            '       ╭──────────────────╮'
            '       │                  │'
            '       │     WAYLAND      │'
            '       │                  │'
            '       ╰──────────────────╯'
            '              ──┴──'
            ''
            ''
        ) ;;
    esac
}

panel_draw() {
    local row line buffer='' page=$((ui_selected/8)) index label number selected hint
    panel_resize
    [ "$ui_active" -eq 1 ] || return 0
    ui_lines=()
    local rule='──────────────────────────────────────────────────────────────────────────'
    ui_lines[0]="${ui_faint}╭${rule}╮${ui_reset}"
    panel_line 1 "B E Z E L                                                   0$ui_stage / 04" "$ui_accent"
    panel_line 2 ''
    panel_line 3 ''
    panel_line 4 "$ui_title" "$ui_accent"
    panel_line 5 ''
    if [ "$ui_kind" = desktop ]; then panel_desktop_logo; else panel_trackpad; fi
    for ((row=0; row<9; row++)); do
        index=$((page*8+row)); label='' selected=0
        if ((row<8 && index<${#menu_labels[@]})); then
            number=$((index+1)); [ "$ui_kind" != action ] || number="${menu_values[$index]}"
            printf -v label '%2s  %s' "$number" "${menu_labels[$index]}"
            if ((index==ui_selected)); then label="› $label"; selected=1; else label="  $label"; fi
        elif ((row==8 && ${#menu_labels[@]}>8)); then label="  $((page+1)) / $(( (${#menu_labels[@]}+7)/8 ))"; fi
        panel_columns "$((row+6))" "${pad_lines[$row]}" "$label" "$selected"
    done
    panel_line 15 ''
    panel_line 16 ''
    if [[ "$ui_preview" = gesture || "$ui_preview" = pose || "$ui_preview" = fingers ]]; then
        local fingers_label="$ui_fingers fingers"
        [ "$ui_fingers" != 1 ] || fingers_label='1 finger'
        hint="${ui_edge^} · $fingers_label"
        [ "$ui_preview" = fingers ] || hint+=" · ${ui_gesture//_/ }"
        panel_line 15 "$hint"
    fi
    if [ "$ui_preview" = gesture ] && [ "$ui_gesture" = hold ]; then
        local fill=$((ui_frame>8 ? 8 : ui_frame)) meter='' n
        for ((n=0;n<8;n++)); do if ((n<fill)); then meter+='━'; else meter+='─'; fi; done
        panel_line 16 "[$meter]" "$ui_accent"
    elif [ "$ui_kind" = review ] && [ "$ui_preview" = gesture ]; then
        panel_line 16 "$ui_action" "$ui_accent"
    fi
    panel_line 17 "$ui_notice"
    panel_line 18 "${ui_digits:+Choice: $ui_digits}"
    hint='↑↓ / #  Move    Enter  Select    Esc  Back'
    [ "$ui_preview" != gesture ] || hint+='    R  Replay'
    panel_line 19 "$hint"
    panel_line 20 ''
    ui_lines[21]="${ui_faint}╰${rule}╯${ui_reset}"
    if [ "$ui_kind" = welcome ]; then
        for ((row=5;row<=17;row++)); do panel_line "$row" ''; done
        # Use the original wordmark, sized to the panel rather than the viewport.
        for row in "${!welcome_logo[@]}"; do panel_line "$((row+6))" "${welcome_logo[$row]}" "$ui_soft"; done
        panel_line 14 'Four edges. Your rules.' "$ui_accent"
        panel_line 15 'A small control surface. Made yours.'
        panel_line 17 "› $((ui_selected+1))  ${menu_labels[$ui_selected]}" "$ui_accent"
    fi
    if [ "$ui_editing" -eq 1 ]; then
        local start=0 visible
        ((ui_cursor<65)) || start=$((ui_cursor-64))
        visible="${ui_text:$start:68}"
        panel_line 18 "$visible" "$ui_accent"
        panel_line 19 'Enter  Confirm    Esc  Back    Ctrl+U  Clear'
    fi
    for ((row=0;row<22;row++)); do
        line="${ui_lines[$row]}"
        if ((row>=6 && row<=14)) && [ "$ui_kind" != welcome ]; then
            local glow="$ui_accent"
            ((ui_frame>1)) || glow="$ui_halo"
            line="${line//┃/$glow┃$ui_soft}"
            line="${line//━/$glow━$ui_soft}"
            line="${line//●/$ui_accent●$ui_soft}"
        fi
        ui_lines[row]="$line"
        if [ "${ui_previous[$row]:-}" != "$line" ]; then
            printf -v line '\033[%d;%dH%s' "$((ui_top+row))" "$ui_left" "$line"
            buffer+="$line"
        fi
    done
    ui_previous=("${ui_lines[@]}")
    if [ "$ui_editing" -eq 1 ]; then
        printf -v line '\033[%d;%dH\033[?25h' "$((ui_top+18))" "$((ui_left+2+ui_cursor-start))"
        buffer+="$line"
    fi
    printf '%s' "$buffer" >&2
}

panel_key() {
    local timeout="${1:-}" ch='' next='' sequence='' status=0
    local -a args=(-s -N 1 -u "$ui_fd")
    [ -z "$timeout" ] || args+=(-t "$timeout")
    if [ -n "$ui_pending" ]; then ch="$ui_pending"; ui_pending=''
    else IFS= read -r "${args[@]}" ch || status=$?; fi
    ui_key=''
    if ((status!=0)); then
        if ((status==1)); then exit 130; fi
        return 0
    fi
    case "$ch" in
        $'\033')
            if IFS= read -r -s -N 1 -t 0.04 -u "$ui_fd" next; then
                if [[ "$next" = '[' || "$next" = O ]]; then
                    while IFS= read -r -s -N 1 -t 0.04 -u "$ui_fd" next; do
                        sequence+="$next"
                        [[ "$next" = [A-Za-z~] ]] && break
                        ((${#sequence}<8)) || break
                    done
                    case "$sequence" in A) ui_key=up ;; B) ui_key=down ;; C) ui_key=right ;; D) ui_key=left ;; H|1~) ui_key=home ;; F|4~) ui_key=end ;; 3~) ui_key=delete ;; esac
                else ui_key=back; ui_pending="$next"; fi
            else ui_key=back; fi ;;
        $'\n'|$'\r') ui_key=enter ;;
        $'\177'|$'\b') ui_key=erase ;;
        $'\025') ui_key=clear ;;
        $'\004') exit 130 ;;
        *) ui_key="$ch" ;;
    esac
}

panel_menu() {
    ui_title="$1" ui_kind="$2" ui_selected="${3:-0}" ui_digits='' ui_frame=0
    ui_notice="$ui_feedback"; ui_feedback=''
    ((ui_selected<${#menu_values[@]})) || ui_selected=0
    [ "$animation" -eq 1 ] || ui_frame=8
    local previous=-1 index number
    while :; do
        if ((previous!=ui_selected)); then
            panel_context; previous=$ui_selected
            ui_frame=8; if [ "$animation" -eq 1 ] && [ "$ui_preview" = gesture ]; then ui_frame=0; fi
        fi
        panel_draw
        if [ "$ui_active" -eq 0 ]; then
            printf '\n%s\n' "$ui_title" >&2
            for index in "${!menu_labels[@]}"; do
                number=$((index+1)); [ "$ui_kind" != action ] || number="${menu_values[$index]}"
                printf ' %s) %s\n' "$number" "${menu_labels[$index]}" >&2
            done
            printf 'Choice [Enter keeps selection; b = back]: ' >&2
            IFS= read -r -u "$ui_fd" answer || exit 130
            [ "$answer" != b ] || return 1
            if [ -z "$answer" ]; then answer="${menu_values[$ui_selected]}"; return 0; fi
            ui_digits="$answer" ui_key=enter
        else
            if ((ui_frame<8)); then panel_key 0.09; else panel_key; fi
        fi
        case "$ui_key" in
            up) ui_selected=$(( (ui_selected+${#menu_values[@]}-1)%${#menu_values[@]} )); ui_digits='' ;;
            down) ui_selected=$(( (ui_selected+1)%${#menu_values[@]} )); ui_digits='' ;;
            back) return 1 ;;
            enter)
                if [ -n "$ui_digits" ]; then
                    local found=0
                    for index in "${!menu_values[@]}"; do
                        number=$((index+1)); [ "$ui_kind" != action ] || number="${menu_values[$index]}"
                        if [ "$ui_digits" = "$number" ]; then ui_selected=$index; found=1; break; fi
                    done
                    if [ "$found" -eq 0 ]; then ui_notice='Choose one of the displayed numbers.'; ui_digits=''; continue; fi
                fi
                answer="${menu_values[$ui_selected]}"; ui_notice=''; return 0 ;;
            r|R) ui_frame=8; if [ "$animation" -eq 1 ] && [ "$ui_preview" = gesture ]; then ui_frame=0; fi ;;
            erase) ui_digits="${ui_digits%?}" ;;
            [0-9])
                if ((${#ui_digits}>=3)); then ui_digits=''; fi
                ui_digits+="$ui_key"
                for index in "${!menu_values[@]}"; do
                    number=$((index+1)); [ "$ui_kind" != action ] || number="${menu_values[$index]}"
                    if [ "$number" = "$ui_digits" ]; then ui_selected=$index; break; fi
                done ;;
            '') if ((ui_frame<8)); then ui_frame=$((ui_frame+1)); fi ;;
        esac
    done
}

panel_input() {
    local prompt="$1"
    ui_notice=''
    ui_text="$2" ui_cursor=${#2} ui_editing=1
    ui_title="$prompt" ui_frame=8
    while :; do
        panel_draw
        if [ "$ui_active" -eq 0 ]; then
            printf '\n%s\nCurrent: %s\n[Enter keeps; b = back]: ' "$prompt" "$ui_text" >&2
            IFS= read -r -u "$ui_fd" answer || exit 130
            ui_editing=0
            [ "$answer" != b ] || return 1
            answer="${answer:-$ui_text}"
            return 0
        fi
        panel_key
        case "$ui_key" in
            enter) answer="$ui_text"; ui_editing=0; printf '\033[?25l' >&2; return 0 ;;
            back) ui_editing=0; printf '\033[?25l' >&2; return 1 ;;
            left) ((ui_cursor==0)) || ui_cursor=$((ui_cursor-1)) ;;
            right) ((ui_cursor==${#ui_text})) || ui_cursor=$((ui_cursor+1)) ;;
            home) ui_cursor=0 ;;
            end) ui_cursor=${#ui_text} ;;
            erase) if ((ui_cursor>0)); then ui_text="${ui_text:0:ui_cursor-1}${ui_text:ui_cursor}"; ui_cursor=$((ui_cursor-1)); fi ;;
            delete) ui_text="${ui_text:0:ui_cursor}${ui_text:ui_cursor+1}" ;;
            clear) ui_text='' ui_cursor=0 ;;
            up|down|'') ;;
            *) if [[ "$ui_key" != [[:cntrl:]] ]]; then ui_text="${ui_text:0:ui_cursor}$ui_key${ui_text:ui_cursor}"; ui_cursor=$((ui_cursor+1)); fi ;;
        esac
    done
}
panel_actions() {
    local id initial=0
    menu_labels=('Leave unused') menu_values=(0)
    for id in {1..30}; do
        if [ -n "$(action_command "$id")" ]; then
            menu_labels+=("$(action_name "$id")"); menu_values+=("$id")
        fi
    done
    if [ "${1:-}" = custom ]; then menu_labels+=('Custom shell command'); menu_values+=(31); fi
    for id in "${!menu_values[@]}"; do [ "${menu_values[$id]}" != "${2:-0}" ] || initial=$id; done
    panel_menu 'Choose what happens' action "$initial"
}

panel_desktop() {
    local -a desktops=(hyprland niri sway plasma gnome generic)
    local initial=0 i key
    for i in "${!desktops[@]}"; do [ "${desktops[$i]}" != "$desktop" ] || initial=$i; done
    while :; do
        menu_labels=(Hyprland Niri Sway 'KDE Plasma' GNOME 'Other Wayland desktop' NixOS)
        menu_values=("${desktops[@]}" nixos)
        panel_menu 'Your desktop' desktop "$initial" || return 1
        [ "$answer" = nixos ] || break
        menu_labels=(Hyprland Niri Sway 'KDE Plasma' GNOME 'Other Wayland desktop')
        menu_values=("${desktops[@]}")
        if panel_menu 'NixOS / Your desktop' desktop "$initial"; then
            declarative=1
            break
        fi
    done
    desktop="$answer"
    configure_desktop
    # Drop compositor-specific presets that are unavailable on the new desktop.
    for key in "${!bindings[@]}"; do
        [ -n "$(action_command "${bindings[$key]}")" ] || unset 'bindings[$key]'
    done
}

panel_edges() {
    local edge_index="${1:-0}" direction key setting
    local -a edges=(left right top bottom)
    while ((edge_index<4)); do
        ui_edge="${edges[$edge_index]}" ui_fingers=1
        menu_labels=('Keep bindings and continue' 'Customize this edge' 'Clear this edge')
        menu_values=(keep edit clear)
        ui_notice=''
        if ! panel_menu "${ui_edge^^} / EDGE CONTROLS" edge_controls; then
            if ((edge_index==0)); then return 1; fi
            edge_index=$((edge_index-1)); continue
        fi
        case "$answer" in
            keep) edge_index=$((edge_index+1)) ;;
            clear)
                for direction in up down left right tap; do unset 'bindings[$ui_edge.$direction]'; done
                for key in "${!advanced_commands[@]}"; do
                    if [[ "$key" = "$ui_edge".* ]]; then unset 'advanced_commands[$key]'; fi
                done
                for setting in "${!advanced_tuning[@]}"; do
                    if [[ "$setting" = "$ui_edge".* ]]; then unset 'advanced_tuning[$setting]'; fi
                done
                ui_action='Edge cleared'; ui_feedback='Edge cleared.'  ;;
            edit) panel_binding edit "$ui_edge" ;;

        esac
    done
}

panel_tuning() {
    local optional="$1" setting current
    local -a fields=("${@:2}")
    while :; do
        menu_labels=() menu_values=("${fields[@]}" "done")
        for setting in "${fields[@]}"; do
            current="${recognition[$setting]}"
            if [ -n "$optional" ]; then current="${advanced_tuning[$key.$setting]:-$current}"; fi
            menu_labels+=("$setting / $current")
        done
        menu_labels+=('Done')
        panel_menu 'Optional tuning' tuning "${#fields[@]}" || return 0
        [ "$answer" != "done" ] || return 0
        setting="$answer" current="${recognition[$answer]}"
        if [ -n "$optional" ]; then current="${advanced_tuning[$key.$setting]:-$current}"; fi
        if read_setting "$setting" "$current" "$optional"; then
            if [ -z "$answer" ]; then continue; fi
            if [ -n "$optional" ]; then
                if [ "$answer" = d ]; then unset 'advanced_tuning[$key.$setting]'; else advanced_tuning[$key.$setting]="$answer"; fi
            else recognition[$setting]="$answer"; fi
            ui_feedback='✓ Sensitivity updated.'
        fi
    done
}

panel_binding() {
    local operation="$1" state=0 key='' advanced_edge="${2:-left}" fingers="${3:-1}" gesture="${4:-tap}" cmd
    local fixed_edge="${2:-}" existing id selected_action direction label
    if [ -n "$fixed_edge" ]; then state=1; ui_edge="$fixed_edge"; fi
    if [ -n "${4:-}" ]; then state=3; key="$advanced_edge.$fingers.$gesture"; fi
    local -a tuning_keys=()
    while :; do
        case "$state" in
            0)
                menu_labels=(Left Right Top Bottom) menu_values=(left right top bottom)
                if ! panel_menu 'Where does it begin?' edge; then return 0; fi
                advanced_edge="$answer" state=1 ;;
            1)
                menu_labels=('One finger' 'Two fingers' 'Three fingers' 'Four fingers') menu_values=(1 2 3 4)
                if ! panel_menu "${advanced_edge^} / How many fingers?" fingers "$((fingers-1))"; then
                    [ -z "$fixed_edge" ] || return 0
                    state=0; continue
                fi
                fingers="$answer" state=2 ;;
            2)
                menu_values=("${gesture_names[@]}") menu_labels=()
                for gesture in "${gesture_names[@]}"; do
                    label="${gesture//_/ }"
                    existing="${advanced_commands[$advanced_edge.$fingers.$gesture]:-}"
                    if [ -z "$existing" ] && [ "$fingers" = 1 ]; then
                        direction="${gesture#swipe_}"
                        if [ -n "${bindings[$advanced_edge.$direction]:-}" ]; then
                            label+=" / $(action_name "${bindings[$advanced_edge.$direction]}")"
                        fi
                    elif [ -n "$existing" ]; then
                        label+=" / $existing"
                        for id in {1..30}; do
                            if [ "$(action_command "$id")" = "$existing" ]; then label="${gesture//_/ } / $(action_name "$id")"; break; fi
                        done
                    fi
                    menu_labels+=("$label")
                done
                menu_labels+=('Done with this edge'); menu_values+=("done")
                if ! panel_menu "${advanced_edge^} / $fingers finger(s)" gesture; then state=1; continue; fi
                [ "$answer" != "done" ] || return 0
                gesture="$answer" key="$advanced_edge.$fingers.$answer"
                if [ "$operation" = delete ]; then
                    remove_advanced_binding; ui_feedback='✓ Binding removed.'; return 0
                fi
                state=3 ;;
            3)
                existing="${advanced_commands[$key]:-}" selected_action=0
                if [ -z "$existing" ] && [ "$fingers" = 1 ]; then
                    direction="${gesture#swipe_}"
                    selected_action="${bindings[$advanced_edge.$direction]:-0}"
                elif [ -n "$existing" ]; then
                    selected_action=31
                    for id in {1..30}; do
                        if [ "$(action_command "$id")" = "$existing" ]; then selected_action="$id"; break; fi
                    done
                fi
                if ! panel_actions custom "$selected_action"; then state=2; continue; fi
                if [ "$answer" = 0 ]; then remove_advanced_binding; ui_feedback='✓ Binding cleared.'; return 0; fi
                if [ "$answer" = 31 ]; then
                    ui_input_initial="${advanced_commands[$key]:-}"
                    if ! ask 'Shell command'; then ui_input_initial=''; continue; fi
                    ui_input_initial=''
                    if [[ ! "$answer" =~ [^[:space:]] ]]; then ui_notice='Enter a nonempty shell command.'; continue; fi
                    cmd="$answer"
                else cmd="$(action_command "$answer")"; fi
                advanced_commands[$key]="$cmd" ui_action="$cmd" ui_feedback='✓ Binding assigned.'
                case "$gesture" in
                    tap) tuning_keys=(tap_distance tap_ms) ;;
                    double_tap) tuning_keys=(tap_distance tap_ms double_tap_ms) ;;
                    hold) tuning_keys=(tap_distance hold_ms) ;;
                    slide_*) tuning_keys=(step_distance) ;;
                    swipe_*) tuning_keys=(swipe_distance) ;;
                esac
                panel_tuning optional "${tuning_keys[@]}"
                if [ -n "$fixed_edge" ] && [ -z "${4:-}" ]; then state=2; else return 0; fi ;;
        esac
    done
}

panel_advanced() {
    while :; do
        menu_labels=('Continue to review' 'Add or edit a binding' 'Delete a binding' 'Shared sensitivity')
        menu_values=(review edit delete sensitivity)
        panel_menu 'Make it yours' advanced || return 1
        case "$answer" in
            review) return 0 ;;
            edit|delete) panel_binding "$answer" ;;
            sensitivity) panel_tuning '' "${recognition_keys[@]}" ;;
        esac
    done
}

panel_review() {
    local edge direction key selection=0
    while :; do
        if [ "$declarative" -eq 1 ]; then
            menu_labels=('Print Home Manager configuration' 'Back to bindings' Cancel)
            menu_values=(1 back 3)
        else
            menu_labels=('Save preview only' 'Apply configuration' 'Back to bindings' Cancel)
            menu_values=(2 1 back 3)
        fi
        for edge in left right top bottom; do
            for direction in up down left right tap; do
                key="$edge.$direction"
                # An explicit one-finger binding wins over a legacy binding.
                local gesture="swipe_$direction"
                [ "$direction" != tap ] || gesture=tap
                if [ -n "${bindings[$key]:-}" ] && [ -z "${advanced_commands[$edge.1.$gesture]:-}" ]; then
                    menu_labels+=("$edge / 1 / $direction / $(action_name "${bindings[$key]}")")
                    menu_values+=("legacy:$key")
                fi
            done
        done
        # Enumerating the schema keeps review order stable across runs.
        local fingers gesture
        for edge in left right top bottom; do
            for fingers in 1 2 3 4; do
                for gesture in "${gesture_names[@]}"; do
                    key="$edge.$fingers.$gesture"
                    if [ -n "${advanced_commands[$key]:-}" ]; then
                        menu_labels+=("$edge / $fingers / ${gesture//_/ }")
                        menu_values+=("binding:$key")
                    fi
                done
            done
        done
        panel_menu 'Review controls / Select a binding to edit' review "$selection" || return 1
        case "$answer" in
            back) return 1 ;;
            3) panel_cleanup; printf 'Cancelled; configuration untouched.\n'; exit 0 ;;
            1|2) save_choice="$answer"; return 0 ;;
            binding:*)
                local selected_key="${answer#binding:}" edge fingers gesture
                IFS=. read -r edge fingers gesture <<< "$selected_key"
                panel_binding edit "$edge" "$fingers" "$gesture"
                selection=0 ;;
            *) selection=$ui_selected ;;
        esac
    done
}

panel_wizard() {
    local stage=0
    if [ "$editing_existing" -eq 1 ]; then stage=3; fi
    if [ "$editing_existing" -eq 0 ]; then
        menu_labels=('Begin setup' Cancel) menu_values=(begin cancel)
        if ! panel_menu 'Your trackpad, tuned to you.' welcome || [ "$answer" = cancel ]; then exit 0; fi
    fi
    while :; do
        case "$stage" in
            0)
                ui_stage=1
                if ! panel_desktop; then
                    menu_labels=('Continue setup' Cancel) menu_values=(continue cancel)
                    if panel_menu 'Leave setup?' welcome && [ "$answer" = cancel ]; then exit 0; fi
                    continue
                fi
                if [ "$panel_defaults_set" -eq 0 ]; then set_desktop_defaults; panel_defaults_set=1; fi
                stage=1 ;;
            1) ui_stage=2; if panel_edges; then stage=2; else stage=0; fi ;;
            2) ui_stage=3; if panel_advanced; then stage=3; else stage=1; fi ;;
            3) ui_stage=4; if panel_review; then return 0; else stage=2; fi ;;
        esac
    done
}

# The original welcome wordmark is shared by both presentations.
mapfile -t welcome_logo <<'ART'
░▒▓███████▓▒░░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░             ░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░           ░▒▓██▓▒░░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓███████▓▒░░▒▓██████▓▒░    ░▒▓██▓▒░  ░▒▓██████▓▒░ ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░       ░▒▓██▓▒░    ░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓█▓▒░░▒▓█▓▒░▒▓█▓▒░      ░▒▓█▓▒░      ░▒▓█▓▒░      ░▒▓█▓▒░
░▒▓███████▓▒░░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░▒▓████████▓▒░
ART
save_choice='' panel_defaults_set=0
load_existing
if panel_init; then
    panel_wizard
    panel_cleanup
    ui_frontend=0
else
    plain_wizard
fi

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
    printf '  };\n'
    emit_advanced nix
    printf '};\n'
    printf '# Also enable hardware.uinput and add your user to the input and uinput groups in NixOS.\n' >&2
    exit 0
fi

mkdir -p "$config_dir"
generated="$(mktemp "$config_dir/.config.toml.XXXXXX")"
{
    printf '# Generated by Bezel onboarding for %s.\n' "$desktop"
    printf '[zones]\nleft_width = 0.08\nright_width = 0.08\ntop_height = 0.08\nbottom_height = 0.08\n'
    emit_bindings toml
    emit_advanced toml
} > "$generated"
if [ "$editing_existing" -eq 1 ]; then
    if ! existing_config "$generated"; then rm -f -- "$generated"; exit 1; fi
fi

if [ -e "$config_file" ] || [ -L "$config_file" ]; then
    printf '\nYour current config is safe at %s.\n' "$config_file" >&2
    printf 'A preview does not change your active gestures. Applying saves your bindings and recognition settings after making a backup. Existing device, zone, and OSD settings are kept.\n' >&2
    if [ -n "$save_choice" ]; then answer="$save_choice"
    elif [ "$input_source" = none ]; then answer=2
    else
        while :; do
            ask '1) Apply now (back up current config)  2) Save preview only  3) Cancel  [Enter = 2]: '
            case "$answer" in ''|1|2|3) break ;; *) echo 'Choose 1, 2, or 3.' >&2 ;; esac
        done
    fi
    case "$answer" in
        ''|2)
            preview="$(mktemp "$config_dir/config.preview.XXXXXX.toml")"
            mv "$generated" "$preview"
            echo "Preview saved to $preview. Your active config is unchanged; run onboarding again and choose 1 to apply it." ;;
        1)
            backup="$(mktemp "$config_file.backup.XXXXXX")"
            cp -p "$config_file" "$backup"
            mv "$generated" "$config_file"
            echo "Saved old config to $backup"
            echo "New config installed at $config_file. Restart Bezel to load it." ;;
        3) rm -- "$generated"; echo 'Cancelled; current config untouched.' ;;
    esac
elif [ "$save_choice" = 2 ]; then
    preview="$(mktemp "$config_dir/config.preview.XXXXXX.toml")"
    mv "$generated" "$preview"
    echo "Preview saved to $preview. No active configuration was created."
else
    mv "$generated" "$config_file"
    echo "Created $config_file for $desktop"
fi
