import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  mkdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const directory = mkdtempSync(join(tmpdir(), "nostrvault-m03-process-"));
const markers = [
  "process fixture password",
  "changed fixture password",
  "lossAcknowledged",
  "vault-system",
];
const containsMarker = (bytes) =>
  markers.some((marker) => bytes.includes(Buffer.from(marker)));
try {
  for (const phase of ["create", "reopen", "change", "new-password"]) {
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "--release",
        "-p",
        "vault-native",
        "--example",
        "vault_lifecycle",
        "--",
        phase,
        directory,
      ],
      { stdio: "inherit" },
    );
  }
  // Positive control is an actual file processed by the same byte scanner.
  writeFileSync(join(directory, "plaintext-control"), markers.join("\n"));
  if (!containsMarker(readFileSync(join(directory, "plaintext-control"))))
    throw new Error("Privacy scanner missed its positive control");
  for (const name of readdirSync(directory)) {
    if (
      name !== "plaintext-control" &&
      containsMarker(readFileSync(join(directory, name)))
    )
      throw new Error(`Plaintext marker in fixture artifact ${name}`);
  }
  const result = execFileSync(
    "cargo",
    [
      "run",
      "--locked",
      "--release",
      "-p",
      "vault-native",
      "--example",
      "vault_benchmark",
    ],
    { encoding: "utf8" },
  );
  const measurement = JSON.parse(result);
  if (
    measurement.milliseconds.length !== 5 ||
    measurement.milliseconds.some((n) => !Number.isFinite(n) || n <= 0)
  )
    throw new Error("KDF measurement failed");
  mkdirSync("test-results", { recursive: true });
  writeFileSync(
    "test-results/m03-native-kdf.json",
    JSON.stringify(measurement, null, 2),
  );
  console.log(result.trim());
  console.log(
    "PASS independent-process persistence, password rewrap, raw SQLite/artifact marker scan with file positive control",
  );
} finally {
  rmSync(directory, { recursive: true, force: true });
}
