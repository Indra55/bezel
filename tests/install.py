"""Exercise installer upgrades with a temporary home and stubbed system commands."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class InstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        shutil.copy2(ROOT / "install.sh", root / "install.sh")
        self.script = root / "install.sh"
        self.home = root / "home"
        self.binary = self.home / ".local/bin/bezel"
        self.binary.parent.mkdir(parents=True)
        self.original = Path(shutil.which("true"))
        self.replacement = root / "replacement"
        self.replacement.write_bytes(self.original.read_bytes() + b"bezel replacement fixture")
        shutil.copy2(self.original, self.binary)
        self.backup = self.binary.with_suffix(".bak")
        config = self.home / ".config/bezel/config.toml"
        config.parent.mkdir(parents=True)
        config.touch()
        self.invalid = root / "invalid-download"
        self.invalid.write_text("partial download")
        self.stubs = root / "bin"
        self.stubs.mkdir()
        self.stub("curl", '''
case "$*" in
    *url_effective*) printf 'https://example.com/releases/tag/v1.1.0\\n' ;;
    */releases/latest/download/*)
        cp -- "$TEST_DOWNLOAD" "${@: -1}"
        exit "$TEST_DOWNLOAD_STATUS" ;;
    *) exit 90 ;;
esac
''')
        self.stub("cargo", '''
[[ "$1" == install && "${@: -2:1}" == --root ]] || exit 91
cp -- "$TEST_REPLACEMENT" "${@: -1}/bin/bezel"
exit "$TEST_BUILD_STATUS"
''')
        self.stub("sudo", "exit 0\n")
        self.stub("systemctl", "exit 0\n")
        self.stub("groups", "printf 'input\\n'\n")

    def stub(self, name, body):
        command = self.stubs / name
        command.write_text("#!/bin/bash\nset -e\n" + body)
        command.chmod(0o755)

    def run_installer(self, download, download_status=0, build_status=0):
        result = subprocess.run(
            ["bash", str(self.script)],
            env={**os.environ, "HOME": str(self.home),
                 "XDG_CONFIG_HOME": str(self.home / ".config"),
                 "PATH": f"{self.stubs}:{self.binary.parent}:{os.environ['PATH']}",
                 "TEST_DOWNLOAD": str(download), "TEST_DOWNLOAD_STATUS": str(download_status),
                 "TEST_REPLACEMENT": str(self.replacement), "TEST_BUILD_STATUS": str(build_status)},
            stdin=subprocess.DEVNULL, capture_output=True, text=True,
            start_new_session=True, timeout=10,
        )
        self.assertEqual(list(self.binary.parent.glob(".bezel-install.*")), [])
        return result

    def test_failed_download_and_build_keep_existing_binary(self):
        self.backup.write_bytes(b"existing backup")
        result = self.run_installer(self.invalid, download_status=22, build_status=42)
        self.assertEqual(result.returncode, 42, result.stderr)
        self.assertEqual(self.binary.read_bytes(), self.original.read_bytes())
        self.assertEqual(self.backup.read_bytes(), b"existing backup")

    def test_invalid_download_keeps_existing_binary(self):
        result = self.run_installer(self.invalid)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Staged Bezel binary", result.stderr)
        self.assertEqual(self.binary.read_bytes(), self.original.read_bytes())
        self.assertFalse(self.backup.exists())

    def test_prebuilt_and_fallback_upgrades_keep_a_backup(self):
        for download, status in [(self.replacement, 0), (self.invalid, 22)]:
            with self.subTest(download_status=status):
                shutil.copy2(self.original, self.binary)
                result = self.run_installer(download, download_status=status)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.binary.read_bytes(), self.replacement.read_bytes())
                self.assertTrue(os.access(self.binary, os.X_OK))
                self.assertEqual(self.backup.read_bytes(), self.original.read_bytes())


if __name__ == "__main__":
    unittest.main()
