import { execFileSync } from "node:child_process";

// Inspect the portable package, not the hosts' intentionally platform-specific graph.
const forbidden = new Set([
  "tauri",
  "rusqlite",
  "jni",
  "web-sys",
  "js-sys",
  "wasm-bindgen",
]);
for (const target of ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"]) {
  const tree = execFileSync(
    "cargo",
    [
      "tree",
      "--locked",
      "-p",
      "vault-core",
      "--target",
      target,
      "--edges",
      "normal",
      "--prefix",
      "none",
    ],
    { encoding: "utf8" },
  );
  for (const line of tree.split("\n")) {
    const name = line.split(" ")[0];
    if (forbidden.has(name))
      throw new Error(`vault-core depends on ${name} for ${target}`);
  }
  console.log(`PASS portable vault-core dependency boundary: ${target}`);
}
