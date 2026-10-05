// Fixture-only Android diagnostics. Never use this collector against a user profile.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const packageName = "com.nostrvault.app";
const fatalPattern =
  /FORTIFY:|Fatal signal|FATAL EXCEPTION|Process crashed|SIGABRT|SIGSEGV|Abort message/;
const fixtureSecrets = [
  "android fixture password",
  "changed android password",
  "foundation fixture password",
  "M04 disposable password",
  ...(process.env.NOSTRVAULT_DIAGNOSTIC_REDACTIONS || "")
    .split("\n")
    .filter(Boolean),
];

export function redactDiagnostic(value) {
  let text = String(value);
  for (const secret of fixtureSecrets)
    text = text.replaceAll(secret, "[REDACTED]");
  return text
    .split("\n")
    .map((line) => {
      // Provider/application output can contain arbitrary payloads. Native fatal
      // messages and lifecycle/system stack frames remain useful without it.
      if (
        /RustStdoutStderr|CONSOLE|console\.(?:log|debug|error)|nostrsigner:/.test(
          line,
        ) &&
        !fatalPattern.test(line)
      )
        return line.replace(/(: ).*$/, "$1[REDACTED application output]");
      return line
        .replace(/\b(?:nsec|ncryptsec)1[a-z0-9]+\b/g, "[REDACTED secret]")
        .replace(
          /NV-(?:PRIVACY|CREDENTIAL)-[A-Za-z0-9_-]+/g,
          "[REDACTED marker]",
        );
    })
    .join("\n");
}

export function attributableFatals(log, crash, instrumentation) {
  const pids = new Set();
  for (const line of log.split("\n")) {
    const thread = line.match(/^\S+\s+\S+\s+(\d+)\s+\d+\s+[VDIWEF]\s+/);
    if (thread && /TestRunner|LifecycleMonitor|ActivityScenario/.test(line))
      pids.add(thread[1]);
    const started = line.match(
      /Start proc (\d+):com\.nostrvault\.app(?:\/|\s)/,
    );
    if (started) pids.add(started[1]);
    const tombstone = line.match(/pid: (\d+).*>>> com\.nostrvault\.app <<</);
    if (tombstone) pids.add(tombstone[1]);
  }
  const failures = [];
  for (const line of (log + "\n" + crash).split("\n")) {
    if (!fatalPattern.test(line)) continue;
    const thread = line.match(/^\S+\s+\S+\s+(\d+)\s+\d+\s+[VDIWEF]\s+/);
    if ((thread && pids.has(thread[1])) || line.includes(packageName))
      failures.push(line);
  }
  if (/Process crashed|INSTRUMENTATION_FAILED|FAILURES/.test(instrumentation))
    failures.push("Instrumentation reported failure/crash");
  return [...new Set(failures)];
}

function hash(path) {
  return existsSync(path)
    ? createHash("sha256").update(readFileSync(path)).digest("hex")
    : "unavailable";
}

export async function diagnoseInstrumentation(
  name,
  command,
  args,
  options = {},
) {
  const serial = process.env.ANDROID_SERIAL;
  if (!serial || !/^emulator-\d+$/.test(serial) || !process.env.ANDROID_HOME)
    throw new Error("Diagnostics require a disposable Android emulator");
  const adb = join(process.env.ANDROID_HOME, "platform-tools/adb");
  const directory = options.directory || join("test-results/android", name);
  mkdirSync(directory, { recursive: true });
  const errors = [];
  const save = (file, value) => {
    try {
      const redacted = redactDiagnostic(value);
      if (fixtureSecrets.some((secret) => redacted.includes(secret)))
        throw new Error("Diagnostic redaction failed");
      writeFileSync(join(directory, file), redacted);
    } catch (error) {
      // Keep collecting other evidence and preserve the original test error.
      errors.push(`Cannot save ${file}: ${error.message}`);
    }
  };
  const call = (adbArgs, optional = false) => {
    const result = spawnSync(adb, ["-s", serial, ...adbArgs], {
      encoding: "utf8",
      timeout: 30000,
      maxBuffer: 32 * 1024 * 1024,
    });
    const value = result.stdout || "";
    if (result.status !== 0) {
      const issue = `unavailable: adb ${adbArgs.join(" ")}: ${result.error?.message || result.stderr || result.status}`;
      if (!optional) errors.push(issue);
      return issue;
    }
    return value;
  };
  call(["logcat", "-b", "all", "-c"]);
  save(
    "activity-before.txt",
    call(["shell", "dumpsys", "activity", "activities"]),
  );
  save(
    "process-before.txt",
    call(["shell", "dumpsys", "activity", "processes"]),
  );
  const apk =
    "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk";
  const library = join(
    process.env.CARGO_TARGET_DIR || "src-tauri/target",
    "x86_64-linux-android/debug/libnostrvault_lib.so",
  );
  const local = (cmd, argv) => {
    const r = spawnSync(cmd, argv, { encoding: "utf8", timeout: 30000 });
    if (r.status === 0) return r.stdout.trim();
    const issue = `unavailable: ${cmd}: ${r.error?.message || r.stderr || r.status}`;
    errors.push(issue);
    return issue;
  };
  const lock = readFileSync("Cargo.lock", "utf8");
  save(
    "versions.txt",
    JSON.stringify(
      {
        commit: local("git", ["rev-parse", "HEAD"]),
        sourceSha256: hash("src-tauri/src/lib.rs"),
        sourceStatus: local("git", ["status", "--short"]),
        collectorSha256: hash("scripts/android-instrumentation.mjs"),
        apkSha256: hash(apk),
        librarySha256: hash(library),
        instrumentationApkSha256: hash(
          "src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug/app-universal-debug-androidTest.apk",
        ),
        libraryElfNotes: local(
          join(
            process.env.NDK_HOME || "",
            "toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf",
          ),
          ["-n", library],
        ),
        tauri: lock.match(/name = "tauri"\nversion = "([^"]+)"/)?.[1],
        wry: lock.match(/name = "wry"\nversion = "([^"]+)"/)?.[1],
        android: call(["shell", "getprop", "ro.build.fingerprint"]),
        api: call(["shell", "getprop", "ro.build.version.sdk"]),
        webview: call(["shell", "dumpsys", "webviewupdate"]),
        sdk: local(
          join(process.env.ANDROID_HOME, "cmdline-tools/latest/bin/sdkmanager"),
          ["--version"],
        ),
        ndk: existsSync(join(process.env.NDK_HOME || "", "source.properties"))
          ? readFileSync(
              join(process.env.NDK_HOME, "source.properties"),
              "utf8",
            )
          : "unavailable",
        gradle: local("src-tauri/gen/android/gradlew", ["--version"]),
        emulator: (() => {
          const path = join(process.env.ANDROID_HOME, "emulator/emulator");
          if (!existsSync(path)) return "unavailable";
          const r = spawnSync(path, ["-version"], {
            encoding: "utf8",
            timeout: 30000,
          });
          return r.status === 0 ? r.stdout.trim() : "unavailable";
        })(),
      },
      null,
      2,
    ),
  );
  let logs = "",
    logError = "",
    output = "",
    primaryError;
  const logger = spawn(
    adb,
    ["-s", serial, "logcat", "-b", "all", "-v", "threadtime"],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  logger.stdout.on("data", (data) => {
    if (logs.length + data.length > 32 * 1024 * 1024) {
      errors.push("Continuous logcat exceeded bound");
      logger.kill();
    } else logs += data.toString();
  });
  logger.stderr.on("data", (data) => {
    logError += data.toString();
  });
  logger.on("error", (error) =>
    errors.push(`Continuous logcat unavailable: ${error.message}`),
  );
  const loggerDone = new Promise((resolve) => logger.on("close", resolve));
  let collecting = true;
  logger.on("close", () => {
    if (collecting)
      errors.push("Continuous logcat stopped before collection completed");
  });
  let crash;
  try {
    await new Promise((resolve, reject) => {
      const child = spawn(command, args, {
        cwd: options.cwd,
        env: process.env,
        stdio: ["ignore", "pipe", "pipe"],
      });
      const timer = setTimeout(() => {
        primaryError = new Error("Instrumentation command timed out");
        child.kill("SIGKILL");
      }, options.timeout || 180000);
      const receive = (data) => {
        if (output.length + data.length > 8 * 1024 * 1024) {
          primaryError = new Error("Instrumentation output exceeded bound");
          child.kill("SIGKILL");
        } else output += data.toString();
      };
      child.stdout.on("data", receive);
      child.stderr.on("data", receive);
      child.on("error", (error) => {
        clearTimeout(timer);
        reject(error);
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        if (primaryError || code !== 0)
          reject(
            primaryError || new Error(`Instrumentation command exited ${code}`),
          );
        else resolve();
      });
    });
  } catch (error) {
    primaryError = error;
  } finally {
    save("instrumentation.txt", output);
    save(
      "activity-after.txt",
      call(["shell", "dumpsys", "activity", "activities"]),
    );
    save(
      "process-after.txt",
      call(["shell", "dumpsys", "activity", "processes"]),
    );
    save(
      "dropbox-tombstones.txt",
      call(
        ["shell", "dumpsys", "dropbox", "--print", "SYSTEM_TOMBSTONE"],
        true,
      ),
    );
    save(
      "dropbox-app-crash.txt",
      call(
        ["shell", "dumpsys", "dropbox", "--print", "SYSTEM_APP_CRASH"],
        true,
      ),
    );
    save(
      "tombstone-access.txt",
      call(["shell", "ls", "-l", "/data/tombstones"], true),
    );
    crash = call(["logcat", "-b", "crash", "-v", "threadtime", "-d"]);
    save("crash-buffer.txt", crash);
    collecting = false;
    logger.kill();
    await loggerDone;
    if (logError) errors.push(`Continuous logcat diagnostic: ${logError}`);
    save("logcat-threadtime.txt", logs);
    save("diagnostic-errors.txt", errors.join("\n"));
  }
  const fatal = attributableFatals(logs, crash, output);
  const bodyPassed =
    /OK \(\d+ tests?\)/.test(output) ||
    /Tests run: \d+, Failures: 0/.test(output) ||
    (options.gradle &&
      /BUILD SUCCESSFUL/.test(output) &&
      logs
        .split("\n")
        .some(
          (line) =>
            /TestRunner\s*: finished:/.test(line) &&
            line.includes(`(${packageName}.${name})`),
        ));
  if (primaryError || errors.length || fatal.length || !bodyPassed)
    throw new Error(
      redactDiagnostic(
        `Android ${name} failed: ${primaryError?.message || ""}\ninstrumentation body: ${bodyPassed ? "completed" : "not successful"}\nfatal diagnostic: ${fatal.join("\n") || "none captured"}\ndiagnostic errors: ${errors.join("\n")}\nEvidence: ${directory}`,
      ),
    );
  console.log(`PASS Android ${name}; no attributable fatal diagnostics`);
  return output;
}
