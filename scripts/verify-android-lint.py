"""Run strict unsuppressed audit, then the approved scoped lint configuration."""
import subprocess

from android_lint_policy import ANDROID, ROOT, load_policy, verify_sources, read_issues, verify_issues

policy = load_policy()
verify_sources(policy)
base = [str(ANDROID / "gradlew"), "--project-dir", str(ANDROID), "--no-daemon",
        "--console=plain", "-PabiList=x86_64", "-ParchList=x86_64", "-PtargetList=x86_64",
        ":app:lintUniversalDebug", "-x", ":app:rustBuildUniversalDebug"]
for audit in (True, False):
    report = ANDROID / "app/build/reports" / ("lint-audit.xml" if audit else "lint-reviewed.xml")
    report.unlink(missing_ok=True)
    result = subprocess.run(base + (["-PnostrvaultLintAudit=true"] if audit else []),
                            cwd=ROOT, capture_output=True, text=True)
    output = result.stdout + result.stderr
    log = ROOT / "test-results" / ("android-lint-audit.log" if audit else "android-lint-reviewed.log")
    log.parent.mkdir(exist_ok=True)
    log.write_text(output)
    if (audit and result.returncode != 1) or (not audit and result.returncode != 0):
        raise RuntimeError(f"Unexpected lint exit {result.returncode}; see {log}")
    verify_issues(read_issues(report), policy, audit)
    print("PASS: strict audit found exactly nine approved diagnostics and one known hint"
          if audit else "PASS: scoped Android lint, zero errors/warnings and one known hint")
