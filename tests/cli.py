"""Run after cargo build: python3 tests/cli.py."""
import os
from pathlib import Path
import pty
import shutil
import subprocess
import tempfile
import termios
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CliTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)
        self.binary = self.directory / "bezel"
        shutil.copy2(ROOT / "target/debug/bezel", self.binary)
        self.config = self.directory / "config/bezel/config.toml"
        self.config.parent.mkdir(parents=True)
        self.original = "# keep me\n[device]\npath='auto'\n[zones]\nleft_width=0.12\n[osd]\nenabled=false\n"
        self.config.write_text(self.original)
        self.env = {**os.environ, "HOME": str(self.directory),
                    "XDG_CONFIG_HOME": str(self.config.parent.parent), "TERM": "dumb"}

    def run_cli(self, *args, answers=None, env=None):
        master, slave = pty.openpty()
        attrs = termios.tcgetattr(slave)
        attrs[3] &= ~termios.ECHO
        termios.tcsetattr(slave, termios.TCSANOW, attrs)
        try:
            process = subprocess.Popen(
                [str(self.binary), *args], cwd=self.directory, env=env or self.env,
                stdin=slave if answers is not None else subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
            )
            if answers is not None:
                os.write(master, ("\n".join(answers) + "\n").encode())
            try:
                stdout, stderr = process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
                self.fail("CLI did not finish: " + stderr.decode()[-2000:])
            return process.returncode, stdout.decode(), stderr.decode()
        finally:
            os.close(master)
            os.close(slave)

    def test_help_version_and_unknown_options_do_not_start_daemon(self):
        for args, expected in [(('--help',), '--configure'), (('-h',), 'Usage:'),
                               (('--configure', '--help'), '--plain'),
                               (('--version',), 'bezel v'), (('-V',), 'bezel v')]:
            with self.subTest(args=args):
                code, stdout, stderr = self.run_cli(*args)
                self.assertEqual(code, 0, stderr)
                self.assertIn(expected, stdout)
                self.assertNotIn('Starting Bezel daemon', stderr)
        for args in [('--unknown',), ('--version', '--unknown')]:
            code, stdout, stderr = self.run_cli(*args)
            self.assertNotEqual(code, 0)
            self.assertIn('bezel --help', stderr)
            self.assertNotIn('Starting Bezel daemon', stderr)
        self.assertEqual(self.config.read_text(), self.original)

    def test_embedded_studio_preserves_existing_config_when_cancelled(self):
        code, stdout, stderr = self.run_cli(
            '--configure', '--plain', '--no-color', '--toml', '--desktop', 'generic',
            answers=['4', '3'],
        )
        self.assertEqual(code, 0, stderr)
        self.assertIn('Cancelled', stdout)
        self.assertEqual(self.config.read_text(), self.original)
        self.assertEqual(list(self.config.parent.iterdir()), [self.config])

    def test_embedded_studio_applies_binding_and_backs_up_existing_config(self):
        code, stdout, stderr = self.run_cli(
            '--configure', '--plain', '--no-color', '--toml', '--desktop', 'generic',
            answers=['1', '1', '2', '1', '31', 'printf custom', '', '', '4', '1'],
        )
        self.assertEqual(code, 0, stderr)
        config = tomllib.loads(self.config.read_text())
        self.assertEqual(config['bindings'], [dict(zone='left', fingers=2, gesture='tap',
                                                   action='command', cmd='printf custom')])
        self.assertEqual(config['zones']['left_width'], 0.12)
        self.assertEqual(config['device']['path'], 'auto')
        self.assertFalse(config['osd']['enabled'])
        backups = list(self.config.parent.glob('config.toml.backup.*'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), self.original)

    def test_studio_argument_errors_preserve_exit_status(self):
        code, stdout, stderr = self.run_cli('--configure', '--unknown')
        self.assertEqual(code, 2)
        self.assertIn('Unknown option: --unknown', stderr)
        self.assertEqual(self.config.read_text(), self.original)

    def test_packaged_helper_receives_options_and_preserves_exit_status(self):
        helper = self.directory / 'bezel-onboard'
        helper.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\nexit 37\n')
        helper.chmod(0o755)
        code, stdout, stderr = self.run_cli(
            '--configure', '--plain', '--nixos', env={**self.env, 'PATH': ''},
        )
        self.assertEqual(code, 37, stderr)
        self.assertEqual(stdout.splitlines(), ['--plain', '--nixos'])

    def test_missing_bash_reports_launch_error(self):
        code, stdout, stderr = self.run_cli('--configure', env={**self.env, 'PATH': ''})
        self.assertNotEqual(code, 0)
        self.assertIn('Could not open Gesture Studio', stderr)
        self.assertEqual(self.config.read_text(), self.original)

    def test_config_check_uses_selected_file_and_does_not_start_daemon(self):
        custom = self.directory / 'alternate config.toml'
        custom.write_text(self.original)
        self.config.write_text('invalid default config')
        for args in [('--config', custom.name, '--check-config'),
                     ('--check-config', '--config', str(custom))]:
            code, stdout, stderr = self.run_cli(*args)
            self.assertEqual(code, 0, stderr)
            self.assertIn('Configuration valid: ' + str(custom), stdout)
            self.assertNotIn('Starting Bezel daemon', stderr)
        self.assertEqual(custom.read_text(), self.original)

    def test_invalid_and_missing_configs_fail_validation_and_startup(self):
        for text in ['not toml', '[recognition]\njoin_ms=0\n',
                     "[[bindings]]\nzone='left'\nfingers=5\ngesture='tap'\naction='command'\ncmd='true'\n"]:
            self.config.write_text(text)
            for args in [('--check-config',), ('--config', str(self.config))]:
                code, stdout, stderr = self.run_cli(*args)
                self.assertNotEqual(code, 0)
                self.assertNotIn('Configuration valid', stdout)
                self.assertNotIn('Starting Bezel daemon', stderr)
                self.assertEqual(self.config.read_text(), text)
        self.config.unlink()
        for args in [('--check-config',), ('--config', str(self.config))]:
            code, stdout, stderr = self.run_cli(*args)
            self.assertNotEqual(code, 0)
            self.assertIn('Failed to read config file', stderr)
            self.assertFalse(self.config.exists())

    def test_alternate_config_can_be_edited_without_changing_default(self):
        custom = self.directory / 'alternate config.toml'
        custom.write_text(self.original)
        code, stdout, stderr = self.run_cli(
            '--config', custom.name, '--configure', '--plain', '--no-color',
            '--toml', '--desktop', 'generic', answers=['4', '1'],
        )
        self.assertEqual(code, 0, stderr)
        self.assertIn('recognition', tomllib.loads(custom.read_text()))
        self.assertEqual(self.config.read_text(), self.original)
        backup, = self.directory.glob('alternate config.toml.backup.*')
        self.assertEqual(backup.read_text(), self.original)

    def test_daemon_startup_uses_alternate_config(self):
        custom = self.directory / 'alternate.toml'
        missing_device = self.directory / 'nonexistent-input-device'
        custom.write_text(f"[device]\npath='{missing_device}'\n")
        self.config.write_text('invalid default config')
        code, stdout, stderr = self.run_cli('--config', custom.name)
        self.assertIn('Starting Bezel daemon', stdout + stderr)
        self.assertIn('Failed to open ' + str(missing_device), stdout + stderr)
        self.assertNotIn('Failed to parse config.toml', stderr)

    def test_invalid_flag_combinations_do_not_start_daemon(self):
        for args in [('--config',), ('--config', ''),
                     ('--config', 'a', '--config', 'b'),
                     ('--check-config', '--status'), ('--list-devices', '--configure'),
                     ('--config', 'a', '--list-devices')]:
            with self.subTest(args=args):
                code, stdout, stderr = self.run_cli(*args)
                self.assertNotEqual(code, 0)
                self.assertNotIn('Starting Bezel daemon', stderr)

    def fake_systemctl(self, output, exit_code=0):
        tools = self.directory / 'tools'
        tools.mkdir(exist_ok=True)
        executable = tools / 'systemctl'
        executable.write_text(
            '#!/bin/sh\n'
            '[ "$*" = "--user show bezel.service --no-pager --property=LoadState,ActiveState,SubState,MainPID" ] || exit 99\n'
            "cat <<'OUTPUT'\n" + output + '\nOUTPUT\n'
            + f'exit {exit_code}\n'
        )
        executable.chmod(0o755)
        return {**self.env, 'PATH': str(tools) + ':' + os.environ['PATH']}

    def test_status_reports_active_inactive_and_missing_service(self):
        for active, sub in [('active', 'running'), ('inactive', 'dead'), ('failed', 'failed')]:
            env = self.fake_systemctl(f'LoadState=loaded\nActiveState={active}\nSubState={sub}\nMainPID=0')
            code, stdout, stderr = self.run_cli('--status', env=env)
            self.assertEqual(code, 0, stderr)
            self.assertIn(f'Service: {active} ({sub})', stdout)
            self.assertIn('Configured device: auto', stdout)
            self.assertIn('Configuration: valid', stdout)
        env = self.fake_systemctl('LoadState=not-found\nActiveState=inactive\nSubState=dead\nMainPID=0', 1)
        code, stdout, stderr = self.run_cli('--status', env=env)
        self.assertEqual(code, 0, stderr)
        self.assertIn('is not installed', stdout)

    def test_status_does_not_report_bus_failure_as_inactive(self):
        env = self.fake_systemctl('', 1)
        code, stdout, stderr = self.run_cli('--status', env=env)
        self.assertNotEqual(code, 0)
        self.assertIn('Could not query bezel.service', stderr)
        self.assertNotIn('Service: inactive', stdout)

    def test_status_inspects_alternate_config_and_reports_invalid_config(self):
        custom = self.directory / 'alternate.toml'
        custom.write_text("[device]\npath='/dev/input/event42'\n")
        env = self.fake_systemctl('LoadState=loaded\nActiveState=active\nSubState=running\nMainPID=0')
        code, stdout, stderr = self.run_cli('--config', str(custom), '--status', env=env)
        self.assertEqual(code, 0, stderr)
        self.assertIn('Config inspected: ' + str(custom), stdout)
        self.assertIn('Configured device: /dev/input/event42', stdout)
        custom.write_text('broken')
        code, stdout, stderr = self.run_cli('--config', str(custom), '--status', env=env)
        self.assertNotEqual(code, 0)
        self.assertIn('Service: active (running)', stdout)
        self.assertNotIn('Configuration: valid', stdout)


if __name__ == '__main__':
    unittest.main()
