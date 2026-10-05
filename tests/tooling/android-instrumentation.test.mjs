import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  attributableFatals,
  redactDiagnostic,
  diagnoseInstrumentation,
} from "../../scripts/android-instrumentation.mjs";

test("successful body cannot mask a fatal from its process", () => {
  const log =
    "10-05 12:00:00.001 123 123 I TestRunner: started: fixture\n10-05 12:00:01.001 123 123 I TestRunner: finished: fixture";
  const crash =
    "10-05 12:00:01.002 123 124 F libc : FORTIFY: pthread_mutex_lock called on a destroyed mutex";
  assert.equal(attributableFatals(log, crash, "OK (1 test)").length, 1);
});

test("failed/timed-out instrumentation preserves finally evidence and primary cause", async () => {
  const temporary = mkdtempSync(join(tmpdir(), "nv-diagnostic-unit-"));
  const previous = {
    ANDROID_HOME: process.env.ANDROID_HOME,
    ANDROID_SERIAL: process.env.ANDROID_SERIAL,
  };
  mkdirSync(join(temporary, "platform-tools"));
  writeFileSync(
    join(temporary, "platform-tools/adb"),
    `#!/usr/bin/env node
const args=process.argv.slice(2);
if(args.includes('logcat')&&!args.includes('-c')&&!args.includes('-d')) {
  console.log('10-05 12:00:00.001 123 123 I TestRunner: android fixture password');
  setInterval(()=>{},1000);
} else if(!args.includes('-c')) console.log('fixture diagnostic');
`,
    { mode: 0o755 },
  );
  process.env.ANDROID_HOME = temporary;
  process.env.ANDROID_SERIAL = "emulator-1234";
  try {
    const directory = join(temporary, "failure");
    mkdirSync(join(directory, "process-after.txt"), { recursive: true });
    await assert.rejects(
      diagnoseInstrumentation(
        "FixtureFailure",
        process.execPath,
        ["-e", "process.exit(7)"],
        { directory },
      ),
      (error) =>
        error.message.includes("exited 7") &&
        error.message.includes("Cannot save process-after.txt"),
    );
    assert.ok(
      readFileSync(join(directory, "instrumentation.txt"), "utf8") !==
        undefined,
    );
    const log = readFileSync(join(directory, "logcat-threadtime.txt"), "utf8");
    assert.ok(!log.includes("android fixture password"));
    const timeoutDirectory = join(temporary, "timeout");
    await assert.rejects(
      diagnoseInstrumentation(
        "FixtureTimeout",
        process.execPath,
        ["-e", "setInterval(()=>{},1000)"],
        { directory: timeoutDirectory, timeout: 50 },
      ),
      /Instrumentation command timed out/,
    );
    assert.ok(
      readFileSync(join(timeoutDirectory, "crash-buffer.txt"), "utf8").includes(
        "fixture diagnostic",
      ),
    );
    await assert.rejects(
      diagnoseInstrumentation(
        "FixtureNoTests",
        process.execPath,
        ["-e", "console.log('BUILD SUCCESSFUL')"],
        { directory: join(temporary, "no-tests"), gradle: true },
      ),
      /instrumentation body: not successful/,
    );
  } finally {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("unrelated process crashes do not become app failures", () => {
  const log = "10-05 12:00:00.001 123 123 I TestRunner: started: fixture";
  const crash = "10-05 12:00:01.002 999 999 F libc : Fatal signal 6 (SIGABRT)";
  assert.deepEqual(attributableFatals(log, crash, "OK (1 test)"), []);
});

test("system process-start evidence attributes app fatal without TestRunner", () => {
  const log =
    "10-05 12:00:00.001 1 1 I ActivityManager: Start proc 456:com.nostrvault.app/u0a123 for activity";
  assert.equal(
    attributableFatals(
      log,
      "10-05 12:00:01.002 456 457 F libc : Abort message: fixture",
      "",
    ).length,
    1,
  );
  assert.equal(
    attributableFatals("", "", "INSTRUMENTATION_FAILED: Process crashed")
      .length,
    1,
  );
});

test("redaction positive control removes secrets and provider output, keeps fatal", () => {
  const control =
    "android fixture password M04 disposable password NV-PRIVACY-positive NV-CREDENTIAL-positive nsec1testsecret";
  assert.ok(control.includes("android fixture password"));
  const cleaned = redactDiagnostic(control);
  for (const marker of control
    .split(" ")
    .filter((s) => s.startsWith("NV-") || s.startsWith("nsec")))
    assert.ok(!cleaned.includes(marker));
  assert.ok(!cleaned.includes("android fixture password"));
  assert.ok(!cleaned.includes("M04 disposable password"));
  assert.ok(
    !redactDiagnostic(
      "10-05 12:00:00 1 1 I RustStdoutStderr: arbitrary decrypted body",
    ).includes("arbitrary decrypted body"),
  );
  assert.ok(
    redactDiagnostic(
      "10-05 12:00:00 1 1 I RustStdoutStderr: FORTIFY: destroyed mutex",
    ).includes("FORTIFY: destroyed mutex"),
  );
});
