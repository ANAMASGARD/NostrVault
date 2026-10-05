// Explicit real-signer gate. Never invoked by verify:commit and never downloads artifacts.
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { readFile, mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { setTimeout as delay } from "node:timers/promises";
const cache = process.env.NOSTRVAULT_SIGNER_CACHE;
if (!cache)
  throw new Error(
    "Set NOSTRVAULT_SIGNER_CACHE to the explicitly prepared, pinned artifact cache. No downloads are performed.",
  );
const manifest = JSON.parse(
  await readFile("tests/signers/artifacts.json", "utf8"),
);
for (const artifact of manifest.artifacts) {
  const bytes = await readFile(join(cache, artifact.artifact));
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    artifact.sha256,
    `Artifact hash mismatch: ${artifact.id}`,
  );
}
const build = spawnSync(
  "cargo",
  ["build", "--locked", "-p", "vault-native", "--example", "signer_acceptance"],
  { stdio: "inherit" },
);
if (build.status !== 0) process.exit(build.status || 1);
const directory = await mkdtemp(join(tmpdir(), "nostrvault-real-signers-"));
const children = [];
const report = {
  complete: false,
  artifacts: manifest.artifacts.map(
    ({ id, version, sourceCommit, sha256 }) => ({
      id,
      version,
      sourceCommit,
      sha256,
    }),
  ),
  results: [],
  limitations: [
    "Chromium/browser-extension private decryption disabled: nos2x 2.5.2 logs plaintext; replacement privacy acceptance unverified.",
    "Full Firefox and Android signer matrix and remaining negative/reconnect cases remain unverified.",
  ],
};
try {
  const binary = resolve(cache, "nak-v0.21.0-linux-amd64");
  const env = {
    ...process.env,
    XDG_CONFIG_HOME: directory,
    NOSTR_SECRET_KEY: "0".repeat(63) + "1",
  };
  const relay = "ws://127.0.0.1:17846",
    secret = randomBytes(16).toString("hex");
  const start = (args) => {
    const p = spawn(binary, args, { env, stdio: "ignore" });
    children.push(p);
    return p;
  };
  start([
    "-q",
    "serve",
    "--hostname",
    "127.0.0.1",
    "--port",
    "17846",
    "--eager-auth",
  ]);
  await delay(500);
  start(["-q", "bunker", "--authorized-secrets", secret, relay]);
  await delay(500);
  assert(
    children.every((p) => p.exitCode === null),
    "Local signer fixture exited before acceptance",
  );
  const pairing = `bunker://79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798?relay=${encodeURIComponent(relay)}&secret=${secret}`;
  const result = spawnSync(
    "src-tauri/target/debug/examples/signer_acceptance",
    [join(directory, "vault"), relay, pairing],
    { stdio: "pipe", timeout: 150000 },
  );
  // Only structured results are retained; external signer diagnostics may contain payloads.
  report.results.push({
    product: "nak",
    platform: process.platform,
    operations: [
      "bunker pairing",
      "account confirmation",
      "nip04 decrypt",
      "nip44 decrypt",
      "scoped kind22242 signature and AUTH acknowledgement",
      "local disconnect",
    ],
    passed: result.status === 0,
  });
  assert.equal(
    result.status,
    0,
    "Real NIP46 acceptance subset failed (raw diagnostics redacted)",
  );
  console.log(
    "PASS real nak NIP46 acceptance subset; full M04 signer acceptance remains incomplete.",
  );
} finally {
  for (const child of children) child.kill();
  await Promise.all(
    children.map((p) =>
      p.exitCode !== null
        ? Promise.resolve()
        : new Promise((resolve) => p.once("exit", resolve)),
    ),
  );
  await rm(directory, { recursive: true, force: true });
  await mkdir("test-results", { recursive: true });
  await writeFile(
    "test-results/signers.json",
    JSON.stringify(report, null, 2) + "\n",
  );
}
if (!process.argv.includes("--linux-subset")) {
  console.error(
    "BLOCKED: full real-signer acceptance matrix is incomplete. See test-results/signers.json.",
  );
  process.exitCode = 1;
}
