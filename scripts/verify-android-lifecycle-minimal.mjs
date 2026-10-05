// Bare-builder proof: android-lifecycle-minimal feature, no vault command surface.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { diagnoseInstrumentation } from "./android-instrumentation.mjs";

const serial = process.env.ANDROID_SERIAL;
if (!serial || !/^emulator-\d+$/.test(serial) || !process.env.ANDROID_HOME)
  throw new Error("A disposable Android emulator is required");
if (!process.env.NDK_HOME)
  throw new Error(
    "NDK_HOME is required for the minimal Android lifecycle build",
  );

const root = process.cwd();
const triple = "x86_64-linux-android";
const abi = "x86_64";
const toolchain = join(
  process.env.NDK_HOME,
  "toolchains/llvm/prebuilt/linux-x86_64/bin",
);
const compiler = "x86_64-linux-android24-clang";
const targetDir = join(root, "src-tauri/target/android-lifecycle-minimal");
const envKey = triple.toUpperCase().replaceAll("-", "_");
const linker = join(toolchain, compiler);
const cargoEnv = {
  ...process.env,
  CARGO_TARGET_DIR: targetDir,
  [`CARGO_TARGET_${envKey}_LINKER`]: linker,
  [`CC_${envKey}`]: linker,
  [`AR_${envKey}`]: join(toolchain, "llvm-ar"),
  CC: linker,
  CXX: join(toolchain, "x86_64-linux-android24-clang++"),
};

function run(cmd, args, options = {}) {
  const result = spawnSync(cmd, args, {
    cwd: options.cwd || root,
    env: options.env || cargoEnv,
    encoding: "utf8",
    timeout: options.timeout || 900000,
    stdio: options.stdio || "inherit",
  });
  if (result.status !== 0)
    throw new Error(`${cmd} ${args.join(" ")} failed (${result.status})`);
}

run("npm", ["run", "build"]);
run("cargo", [
  "build",
  "--locked",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--features",
  "android-lifecycle-minimal",
  "--target",
  triple,
]);
const jni = join(root, "src-tauri/gen/android/app/src/main/jniLibs", abi);
mkdirSync(jni, { recursive: true });
copyFileSync(
  join(targetDir, triple, "debug/libnostrvault_lib.so"),
  join(jni, "libnostrvault_lib.so"),
);
const gradleArgs = [
  "--console=plain",
  "-PabiList=x86_64",
  "-ParchList=x86_64",
  "-PtargetList=x86_64",
  ":app:assembleUniversalDebug",
  ":app:assembleUniversalDebugAndroidTest",
  "-x",
  ":app:rustBuildUniversalDebug",
  "-x",
  ":app:rustBuildX86_64Debug",
];
run("./gradlew", gradleArgs, {
  cwd: join(root, "src-tauri/gen/android"),
});

const adb = join(process.env.ANDROID_HOME, "platform-tools/adb");
function adbInstall(args) {
  const result = spawnSync(adb, ["-s", serial, ...args], { stdio: "inherit" });
  if (result.status !== 0)
    throw new Error(`adb install failed: ${args.join(" ")}`);
}
adbInstall([
  "install",
  "-r",
  join(
    root,
    "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk",
  ),
]);
adbInstall([
  "install",
  "-r",
  join(
    root,
    "src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug/app-universal-debug-androidTest.apk",
  ),
]);
for (const spec of [
  "com.nostrvault.app.LifecycleDiagnosisTest",
  "com.nostrvault.app.LifecycleFrameworkTest#closeRecreate",
  "com.nostrvault.app.LifecycleFrameworkTest#freshProcessReopen",
]) {
  const label = spec.replace(/[#.]/g, "-");
  await diagnoseInstrumentation(
    `framework-minimal-${label}`,
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
      spec,
      "com.nostrvault.app.test/androidx.test.runner.AndroidJUnitRunner",
    ],
    { directory: join("test-results/android", `framework-minimal/${label}`) },
  );
}

console.log("PASS Android minimal lifecycle reproducer");
