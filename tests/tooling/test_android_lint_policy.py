import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from android_lint_policy import load_policy, verify_issues, verify_sources, lint_config


class AndroidLintPolicyTest(unittest.TestCase):
    def setUp(self):
        self.policy = load_policy()
        self.issues = [{k: e[k] for k in ("id", "severity", "path", "message")}
                       for e in self.policy["exceptions"]] + self.policy["knownHints"]

    def test_only_exact_reviewed_report_is_accepted(self):
        verify_issues(self.issues, self.policy, True)
        verify_issues(self.policy["knownHints"], self.policy, False)
        with self.assertRaises(ValueError):
            verify_issues(self.issues, self.policy, False)

    def test_same_issue_in_application_code_is_not_excepted(self):
        actual = copy.deepcopy(self.issues)
        actual[0]["path"] = "app/src/main/java/com/nostrvault/app/MainActivity.kt"
        with self.assertRaises(ValueError):
            verify_issues(actual, self.policy, True)

    def test_new_or_missing_findings_require_review(self):
        for actual in (self.issues[:-1], self.issues + [self.issues[0]]):
            with self.assertRaises(ValueError):
                verify_issues(actual, self.policy, True)

    def test_changed_version_pair_requires_review(self):
        actual = copy.deepcopy(self.issues)
        entry = next(e for e in actual if e["id"] == "GradleDependency")
        entry["message"] = entry["message"].replace("1.14.0", "1.15.0")
        with self.assertRaises(ValueError):
            verify_issues(actual, self.policy, True)

    def test_wry_upgrade_requires_review(self):
        with self.assertRaisesRegex(ValueError, "Wry version changed"):
            verify_sources(self.policy, lock='name = "wry"\nversion = "0.58.0"\n')

    def test_generated_source_change_requires_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in self.policy["generatedSha256"]:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("changed generated source")
            with self.assertRaisesRegex(ValueError, "Generated source changed"):
                verify_sources(self.policy, android=root)

    def test_lint_scope_does_not_disable_categories(self):
        config = lint_config(self.policy)
        self.assertNotIn('severity=', config)
        self.assertNotIn('path="*', config)
        self.assertEqual(config.count('<ignore '), 9)


if __name__ == "__main__":
    unittest.main()
