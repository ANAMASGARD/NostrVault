"""Fail-closed audit for the nine maintainer-approved Android lint exceptions."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "src-tauri/gen/android"
POLICY_PATH = ANDROID / "lint-exceptions.json"


def load_policy():
    return json.loads(POLICY_PATH.read_text())


def lint_config(policy):
    root = ET.Element("lint")
    # Android lint matches path OR regexp, not their conjunction. The unsuppressed
    # audit below enforces issue + path + message before scoped lint can pass.
    for entry in policy["exceptions"]:
        issue = ET.SubElement(root, "issue", {"id": entry["id"]})
        if entry["id"] in ("DefaultLocale", "UseKtx", "ViewConstructor"):
            attrs = {"path": entry["path"]}
        else:
            attrs = {"regexp": "^" + re.escape(entry["message"]) + "$"}
        ET.SubElement(issue, "ignore", attrs)
    ET.indent(root, space="  ")
    return '<?xml version="1.0" encoding="UTF-8"?>\n<!-- Approved exceptions; guarded by scripts/verify-android-lint.py. -->\n' + ET.tostring(root, encoding="unicode") + "\n"


def verify_sources(policy, android=ANDROID, lock=None):
    if lock is None:
        lock = (ROOT / "Cargo.lock").read_text()
    if not re.search(r'name = "wry"\nversion = "' + re.escape(policy["wryVersion"]) + r'"\n', lock):
        raise ValueError("Wry version changed: re-review lint exceptions")
    for relative, expected in policy["generatedSha256"].items():
        actual = hashlib.sha256((android / relative).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"Generated source changed: re-review {relative}")
    if (android / "lint-reviewed.xml").read_text() != lint_config(policy):
        raise ValueError("Lint configuration differs from approved finite exceptions")
    if (android / "lint-audit.xml").read_text().strip() != "<lint />":
        raise ValueError("The independent audit must not suppress any issue")


def read_issues(path, android=ANDROID):
    result = []
    for issue in ET.parse(path).getroot().findall("issue"):
        locations = issue.findall("location")
        if len(locations) != 1:
            raise ValueError("Unexpected lint location structure")
        relative = Path(locations[0].attrib["file"]).resolve().relative_to(android.resolve())
        result.append({"id": issue.attrib["id"], "severity": issue.attrib["severity"],
                       "path": relative.as_posix(), "message": issue.attrib["message"]})
    return result


def verify_issues(actual, policy, audit):
    expected = list(policy["knownHints"])
    if audit:
        expected += [{k: e[k] for k in ("id", "severity", "path", "message")}
                     for e in policy["exceptions"]]
    key = lambda entry: json.dumps(entry, sort_keys=True)
    if sorted(map(key, actual)) != sorted(map(key, expected)):
        extra = [i for i in actual if i not in expected]
        missing = [i for i in expected if i not in actual]
        raise ValueError(f"Lint report changed; re-review required. Unexpected: {extra}; absent: {missing}")
