"""Run the diagnostic security regressions in the existing tooling gate."""

import pathlib
import subprocess
import unittest


class AndroidInstrumentationTests(unittest.TestCase):
    def test_node_diagnostic_regressions(self):
        root = pathlib.Path(__file__).resolve().parents[2]
        result = subprocess.run(
            ["node", "--test", "tests/tooling/android-instrumentation.test.mjs"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
