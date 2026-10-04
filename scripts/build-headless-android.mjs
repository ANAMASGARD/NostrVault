import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const target = process.argv[2];
const platform = {
  aarch64: [
    "aarch64-linux-android",
    "arm64-v8a",
    "aarch64-linux-android24-clang",
  ],
  x86_64: ["x86_64-linux-android", "x86_64", "x86_64-linux-android24-clang"],
}[target];
if (!platform || !process.env.NDK_HOME) {
  throw new Error(
    "Provide aarch64 or x86_64 and NDK_HOME for the headless library.",
  );
}
const [triple, abi, compiler] = platform;
const toolchain = join(
  process.env.NDK_HOME,
  "toolchains/llvm/prebuilt/linux-x86_64/bin",
);
const env = {
  ...process.env,
  [`CARGO_TARGET_${triple.toUpperCase().replaceAll("-", "_")}_LINKER`]: join(
    toolchain,
    compiler,
  ),
  [`CC_${triple.replaceAll("-", "_")}`]: join(toolchain, compiler),
  [`AR_${triple.replaceAll("-", "_")}`]: join(toolchain, "llvm-ar"),
};
const result = spawnSync(
  "cargo",
  ["build", "--locked", "-p", "vault-native", "--target", triple, "--release"],
  { env, stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status || 1);
const output = join("src-tauri/gen/android/app/src/main/jniLibs", abi);
mkdirSync(output, { recursive: true });
copyFileSync(
  join("src-tauri/target", triple, "release/libvault_native.so"),
  join(output, "libvault_native.so"),
);
