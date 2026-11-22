"""Exercise the actual wizard in a pseudo-terminal, isolated from user config."""
import os
from pathlib import Path
import pty
import subprocess
import tempfile
import termios
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class OnboardingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.config = Path(self.tmp.name) / "bezel" / "config.toml"

    def run_wizard(self, answers, nix=False, desktop="generic", path=None):
        master, slave = pty.openpty()
        attrs = termios.tcgetattr(slave)
        attrs[3] &= ~termios.ECHO
        termios.tcsetattr(slave, termios.TCSANOW, attrs)
        try:
            process = subprocess.Popen(
                ["bash", str(ROOT / "onboard.sh"), "--desktop", desktop, "--no-color", "--nixos" if nix else "--toml"],
                stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                env={**os.environ, "XDG_CONFIG_HOME": self.tmp.name, "PATH": path or os.environ["PATH"]},
            )
            os.write(master, ("\n".join(answers) + "\n").encode())
            try:
                stdout, stderr = process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
                self.fail("Wizard did not finish: " + stderr.decode()[-2000:])
            self.assertEqual(process.returncode, 0, stderr.decode())
            return stdout.decode()
        finally:
            os.close(master)
            os.close(slave)

    def test_custom_binding_and_override(self):
        cmd = 'printf "%s\\n" "${HOME}" `literal`'
        self.run_wizard(["1"] * 4 + ["1", "1", "2", "12", "31", cmd, "0.04", "4"])
        config = tomllib.loads(self.config.read_text())
        self.assertEqual(config["bindings"], [dict(zone="left", fingers=2, gesture="slide_up", action="command", cmd=cmd, step_distance=0.04)])
        self.assertEqual(config["recognition"]["join_ms"], 80)
        self.assertIn("up", config["gestures"]["left"])

    def test_unused_removes_legacy_binding(self):
        self.run_wizard(["1"] * 4 + ["1", "1", "1", "4", "0", "4"])
        config = tomllib.loads(self.config.read_text())
        self.assertNotIn("up", config["gestures"]["left"])

    def test_preview_cancel_and_apply(self):
        self.config.parent.mkdir()
        original = "# preserve me\n[device]\npath='auto'\n"
        self.config.write_text(original)
        self.run_wizard(["4", "2"])
        self.assertEqual(self.config.read_text(), original)
        previews = list(self.config.parent.glob("config.preview.*.toml"))
        self.assertEqual(len(previews), 1)
        tomllib.loads(previews[0].read_text())
        self.run_wizard(["4", "3"])
        self.assertEqual(self.config.read_text(), original)
        self.run_wizard(["4", "1"])
        self.assertEqual(next(self.config.parent.glob("config.toml.backup.*")).read_text(), original)
        self.assertIn("recognition", tomllib.loads(self.config.read_text()))

    def test_nix_custom_command_escaping(self):
        output = self.run_wizard(["1"] * 4 + ["1", "1", "2", "1", "31", 'echo "${HOME}" \\path', "", "", "4"], nix=True)
        self.assertIn('cmd = "echo \\"\\${HOME}\\" \\\\path";', output)
        self.assertIn('fingers = 2; gesture = "tap";', output)
        self.assertIn("recognition = {", output)
        self.assertFalse(self.config.exists())

    def test_edit_delete_and_shared_tuning(self):
        self.run_wizard(["1"] * 4 + [
            "1", "1", "2", "3", "1", "", "600",  # add hold
            "1", "1", "2", "3", "2", "", "d",    # edit, remove override
            "2", "1", "2", "3",                    # delete hold
            "3", "", "0.07", "", "", "", "", "", # shared sensitivity
            "4",
        ])
        config = tomllib.loads(self.config.read_text())
        self.assertNotIn("bindings", config)
        self.assertEqual(config["recognition"]["swipe_distance"], 0.07)

    def test_new_window_presets(self):
        expected = {
            "hyprland": ["window.close()", "window.fullscreen", "window.float", 'direction = "left"', 'direction = "right"', 'direction = "up"', 'direction = "down"'],
            "niri": ["close-window", "fullscreen-window", "toggle-window-floating", "focus-column-left", "focus-column-right", "focus-window-up", "focus-window-down"],
            "sway": ["kill", "fullscreen toggle", "floating toggle", "focus left", "focus right", "focus up", "focus down"],
        }
        for desktop, commands in expected.items():
            with self.subTest(desktop=desktop):
                self.config.unlink(missing_ok=True)
                answers = ["1"] * 4
                for gesture, action in enumerate(range(17, 24), 4):
                    answers += ["1", "1", "2", str(gesture), str(action), ""]
                self.run_wizard(answers + ["4", "1"], desktop=desktop)
                bindings = tomllib.loads(self.config.read_text())["bindings"]
                self.assertEqual(len(bindings), 7)
                gestures = ["swipe_up", "swipe_down", "swipe_left", "swipe_right", "swipe_up_left", "swipe_up_right", "swipe_down_left"]
                by_gesture = {b["gesture"]: b["cmd"] for b in bindings}
                for gesture, command in zip(gestures, commands):
                    self.assertIn(command, by_gesture[gesture])

    def test_new_utility_presets(self):
        tools = Path(self.tmp.name) / "bin"
        tools.mkdir()
        for name in ("fuzzel", "foot", "hyprlock", "grim", "slurp", "wl-copy", "cliphist", "swaync-client"):
            tool = tools / name
            tool.write_text("#!/bin/sh\nexit 99\n")
            tool.chmod(0o755)
        answers = ["1"] * 4
        for gesture, action in enumerate(range(24, 31), 4):
            answers += ["1", "1", "2", str(gesture), str(action), ""]
        self.run_wizard(answers + ["4"], path=f"{tools}:{os.environ['PATH']}")
        bindings = tomllib.loads(self.config.read_text())["bindings"]
        commands = {b["cmd"] for b in bindings}
        self.assertEqual(len(commands), 7)
        self.assertTrue({"fuzzel", "foot", "hyprlock", "swaync-client --toggle-dnd"} <= commands)
        self.assertTrue(any('grim -g "$region"' in cmd for cmd in commands))
        self.assertIn("grim - | wl-copy --type image/png", commands)
        self.assertTrue(any("cliphist decode | wl-copy" in cmd for cmd in commands))

    def test_unsupported_window_action_is_rejected(self):
        self.run_wizard(["1"] * 4 + ["1", "1", "2", "4", "17", "1", "", "4"])
        binding = tomllib.loads(self.config.read_text())["bindings"][0]
        self.assertEqual(binding["cmd"], "wpctl set-volume @DEFAULT_SINK@ 5%+")

    def test_brave_context_swipes_are_opt_in(self):
        tools = Path(self.tmp.name) / "test tools"
        tools.mkdir()
        wtype = tools / "wtype"
        wtype.write_text("#!/bin/sh\nexit 99\n")
        wtype.chmod(0o755)
        path = f"{tools}:{os.environ['PATH']}"
        self.run_wizard(["1"] * 4 + ["4"], desktop="niri", path=path)
        self.assertNotIn("apps", tomllib.loads(self.config.read_text()))

        original = self.config.read_text() + '''
[[apps."brave-browser".bindings]]
zone = "bottom"
fingers = 1
gesture = "tap"
action = "command"
cmd = "keep Brave tap"

[[apps.firefox.bindings]]
zone = "bottom"
fingers = 1
gesture = "swipe_right"
action = "command"
cmd = "keep Firefox swipe"
'''
        self.config.write_text(original)
        self.run_wizard(["5", "1", "4", "1"], desktop="niri", path=path)
        profiles = tomllib.loads(self.config.read_text())["apps"]
        commands = {b["gesture"]: b["cmd"] for b in profiles["brave-browser"]["bindings"]}
        self.assertEqual(commands, {
            "tap": "keep Brave tap",
            "swipe_left": f"'{wtype}' -M ctrl -P Page_Up -p Page_Up -m ctrl",
            "swipe_right": f"'{wtype}' -M ctrl -P Page_Down -p Page_Down -m ctrl",
        })
        self.assertEqual(profiles["firefox"], tomllib.loads(original)["apps"]["firefox"])

        self.run_wizard(["5", "2", "4", "1"], desktop="niri", path=path)
        self.assertEqual(tomllib.loads(self.config.read_text())["apps"], tomllib.loads(original)["apps"])

    def test_brave_context_swipes_nix_output(self):
        output = self.run_wizard(["1"] * 4 + ["5", "1", "4"], nix=True, desktop="hyprland")
        self.assertIn("playerctl wtype ];", output)
        self.assertIn('apps."brave-browser".bindings = [', output)
        self.assertIn('cmd = "wtype -M ctrl -P Page_Down -p Page_Down -m ctrl";', output)


if __name__ == "__main__":
    unittest.main()
