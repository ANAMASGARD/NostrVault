// Ten create and ten reopen repetitions with the strict diagnostic collector.
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { diagnoseInstrumentation } from "./android-instrumentation.mjs";

const serial = process.env.ANDROID_SERIAL;
if (!serial || !/^emulator-\d+$/.test(serial) || !process.env.ANDROID_HOME)
  throw new Error("A disposable Android emulator is required");

const adb = join(process.env.ANDROID_HOME, "platform-tools/adb");
const repeat = Number(process.env.NOSTRVAULT_ANDROID_LIFECYCLE_REPEAT || "10");
if (!Number.isInteger(repeat) || repeat < 1)
  throw new Error(
    "NOSTRVAULT_ANDROID_LIFECYCLE_REPEAT must be a positive integer",
  );

function install() {
  spawnSync(
    adb,
    [
      "-s",
      serial,
      "install",
      "-r",
      "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk",
    ],
    { stdio: "inherit" },
  );
  spawnSync(
    adb,
    [
      "-s",
      serial,
      "install",
      "-r",
      "src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug/app-universal-debug-androidTest.apk",
    ],
    { stdio: "inherit" },
  );
}

install();

function instrument(className, directory) {
  return diagnoseInstrumentation(
    className,
    adb,
    [
      "-s",
      serial,
      "shell",
      "am",
      "instrument",
      "-w",
      "-r",
      "-e",
      "class",
      className,
      "com.nostrvault.app.test/androidx.test.runner.AndroidJUnitRunner",
    ],
    { directory },
  );
}

for (let index = 1; index <= repeat; index++) {
  spawnSync(adb, [
    "-s",
    serial,
    "shell",
    "am",
    "force-stop",
    "com.nostrvault.app",
  ]);
  spawnSync(adb, ["-s", serial, "shell", "pm", "clear", "com.nostrvault.app"], {
    stdio: "inherit",
  });
  await instrument(
    "com.nostrvault.app.VaultCreateTest",
    join("test-results/android", `repeat/VaultCreateTest-${index}`),
  );
}

for (let index = 1; index <= repeat; index++) {
  spawnSync(adb, [
    "-s",
    serial,
    "shell",
    "am",
    "force-stop",
    "com.nostrvault.app",
  ]);
  spawnSync(adb, ["-s", serial, "shell", "pm", "clear", "com.nostrvault.app"], {
    stdio: "inherit",
  });
  await instrument(
    "com.nostrvault.app.VaultCreateTest",
    join("test-results/android", `repeat/VaultReopenTest-${index}-setup`),
  );
  await instrument(
    "com.nostrvault.app.VaultReopenTest",
    join("test-results/android", `repeat/VaultReopenTest-${index}`),
  );
}

await instrument(
  "com.nostrvault.app.LifecycleFrameworkTest#closeRecreate",
  join("test-results/android", "repeat/LifecycleFrameworkTest-closeRecreate"),
);

await instrument(
  "com.nostrvault.app.LifecycleFrameworkTest#freshProcessReopen",
  join("test-results/android", "repeat/LifecycleFrameworkTest-fresh"),
);

console.log(`PASS Android lifecycle ${repeat}+${repeat} repetitions`);
