import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";

const mode = process.argv[2] ?? "all";
if (!["all", "web", "linux", "android", "web-linux"].includes(mode))
  throw new Error(`Unknown gate: ${mode}`);
const common = [
  ["npm", "run", "build:wasm"],
  ["npm", "run", "format:check"],
  ["npm", "run", "typecheck"],
  ["npm", "run", "lint"],
  ["npm", "run", "test:unit"],
  [
    "python3",
    "-m",
    "unittest",
    "discover",
    "-s",
    "tests/tooling",
    "-p",
    "test_*.py",
  ],
];
common.push(["cargo", "fmt", "--all", "--", "--check"]);
const gates = {
  web: [
    ["node", "scripts/verify-core-boundary.mjs"],
    [
      "cargo",
      "clippy",
      "--locked",
      "-p",
      "vault-core",
      "--all-targets",
      "--",
      "-D",
      "warnings",
    ],
    ["cargo", "test", "--locked", "-p", "vault-core"],
    [
      "cargo",
      "clippy",
      "--locked",
      "-p",
      "vault-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--",
      "-D",
      "warnings",
    ],
    ["npm", "run", "build:web"],
    ["npm", "run", "test:web"],
  ],
  linux: [
    [
      "cargo",
      "clippy",
      "--locked",
      "-p",
      "vault-native",
      "-p",
      "nostrvault",
      "--all-targets",
      "--",
      "-D",
      "warnings",
    ],
    ["cargo", "test", "--locked", "-p", "vault-native", "-p", "nostrvault"],
    [
      "npm",
      "run",
      "tauri",
      "--",
      "build",
      "--bundles",
      "deb,rpm",
      "--",
      "--locked",
    ],
    ["npm", "run", "test:linux"],
  ],
  android: [["node", "scripts/verify-android.mjs"]],
};
const commands = [
  ...common,
  ...(mode === "all"
    ? Object.values(gates).flat()
    : mode === "web-linux"
      ? [...gates.web, ...gates.linux]
      : gates[mode]),
  ["git", "diff", "--check"],
];
const results = [];
for (const [command, ...args] of commands) {
  console.log(`\nGate: ${[command, ...args].join(" ")}`);
  const startedAt = new Date().toISOString();
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env: {
      ...process.env,
      CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "2",
    },
  });
  results.push({
    command: [command, ...args],
    startedAt,
    endedAt: new Date().toISOString(),
    status: result.status,
    error: result.error?.message,
  });
  mkdirSync("test-results", { recursive: true });
  writeFileSync(
    `test-results/gate-${mode}.json`,
    JSON.stringify(
      {
        mode,
        results,
        complete: results.length === commands.length && result.status === 0,
      },
      null,
      2,
    ),
  );
  if (result.status !== 0) {
    console.error(
      `Gate failed: ${command}; ${result.error?.message ?? result.signal ?? result.status}. No milestone commit is permitted.`,
    );
    process.exit(result.status || 1);
  }
}
console.log(
  `PASS ${mode} gates. Human/device/signer requirements still need separate evidence at their assigned milestones.`,
);
