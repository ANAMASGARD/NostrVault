import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";

mkdirSync("src/generated", { recursive: true });
const shared = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "-p",
    "vault-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
  ],
  { stdio: "inherit" },
);
if (shared.status !== 0) process.exit(shared.status || 1);
const bindings = spawnSync(
  "wasm-bindgen",
  [
    "src-tauri/target/wasm32-unknown-unknown/release/vault_wasm.wasm",
    "--target",
    "web",
    "--out-dir",
    "src/generated",
    "--out-name",
    "vault_wasm",
  ],
  { stdio: "inherit" },
);
if (bindings.status !== 0) process.exit(bindings.status || 1);
