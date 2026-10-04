import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";

const result = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "--manifest-path",
    "tools/runtime-probe/Cargo.toml",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--target-dir",
    "target/runtime-probe",
  ],
  { stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status || 1);
mkdirSync("src/generated", { recursive: true });
copyFileSync(
  "target/runtime-probe/wasm32-unknown-unknown/release/nostrvault_runtime_probe.wasm",
  "src/generated/runtime_probe.wasm",
);
