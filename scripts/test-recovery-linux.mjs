// Explicit disposable Linux UI recovery acceptance. No artifact downloads or public relays.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
const nak = process.env.NOSTRVAULT_NAK;
assert(
  nak,
  "Set NOSTRVAULT_NAK to the pinned nak 0.21.0 binary; no downloads occur.",
);
assert.equal(
  createHash("sha256")
    .update(await readFile(nak))
    .digest("hex"),
  "7bf6d8d82a9e9cf9aca74a04449fb2235bc49624a73b3539cf7d6f95304837a1",
);
const root = await mkdtemp(join(tmpdir(), "nv-recovery-fixture-"));
const children = [];
const env = {
  ...process.env,
  XDG_CONFIG_HOME: root,
  XDG_DATA_HOME: root,
  NOSTR_SECRET_KEY: "0".repeat(63) + "1",
};
const a = "ws://127.0.0.1:17951",
  b = "ws://127.0.0.1:17952",
  c = "ws://127.0.0.1:17953";
const account =
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
function command(args, input) {
  const r = spawnSync(
    nak,
    ["-q", ...args, ...(args[0] === "event" ? ["--github"] : [])],
    {
      env,
      input: input ?? (args[0] === "req" ? "{}\n" : undefined),
      encoding: "utf8",
      timeout: 20000,
    },
  );
  assert.equal(r.status, 0, "Fixture nak command failed");
  return r.stdout.trim();
}
function start(binary, args, childEnv = env) {
  const p = spawn(binary, args, { env: childEnv, stdio: "ignore" });
  children.push(p);
  return p;
}
let session;
async function request(path, body, method = "POST") {
  const response = await fetch(`http://127.0.0.1:4445${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(20000),
  });
  const result = await response.json();
  assert(response.ok && !result.value?.error, "Native UI automation failed");
  return result.value;
}
async function script(source, args = []) {
  return request(`/session/${session}/execute/sync`, { script: source, args });
}
async function wait(expression) {
  for (let i = 0; i < 160; i++) {
    if (await script(`return ${expression}`)) return;
    await delay(250);
  }
  throw new Error(`Expected UI state timed out: ${expression}`);
}
async function click(label) {
  await wait(
    `Array.from(document.querySelectorAll('button')).some(b=>b.textContent===${JSON.stringify(label)} && !b.disabled)`,
  );
  await script(
    "Array.from(document.querySelectorAll('button')).find(b=>b.textContent===arguments[0]).click()",
    [label],
  );
}
async function fill(label, value) {
  await script(
    "const l=Array.from(document.querySelectorAll('label')).find(l=>l.firstChild.textContent.trim()===arguments[0]); const i=l.querySelector('input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(i,arguments[1]); i.dispatchEvent(new Event('input',{bubbles:true}));",
    [label, value],
  );
}
async function open() {
  const result = await request("/session", {
    capabilities: {
      alwaysMatch: {
        browserName: "wry",
        "tauri:options": {
          application: resolve("src-tauri/target/release/nostrvault"),
        },
      },
    },
  });
  session = result.sessionId;
  await wait("document.querySelector('label') !== null");
}
const report = { fixture: true, expected: 11, passed: false, observations: [] };
try {
  const now = Math.floor(Date.now() / 1000) - 5;
  const notes = Array.from({ length: 12 }, (_, i) =>
    JSON.parse(
      command([
        "event",
        "-c",
        `NV_PUBLIC_RECOVERY_FIXTURE_${i}`,
        "--created-at",
        String(now),
      ]),
    ),
  );
  const expired = JSON.parse(
    command([
      "event",
      "-c",
      "NV_EXPIRED_FIXTURE",
      "--created-at",
      String(now),
      "-t",
      `expiration=${now - 1}`,
    ]),
  );
  const protectedNote = JSON.parse(
    command([
      "event",
      "-c",
      "NV_PROTECTED_FIXTURE",
      "--created-at",
      String(now),
      "-t",
      "-",
    ]),
  );
  const deletion = JSON.parse(
    command([
      "event",
      "-k",
      "5",
      "-c",
      "disposable deletion",
      "--created-at",
      String(now),
      "-e",
      notes[0].id,
    ]),
  );
  const expected = notes
    .slice(1)
    .map((e) => e.id)
    .sort();
  await writeFile(
    join(root, "events.jsonl"),
    [...notes, expired, protectedNote, deletion]
      .map((e) => JSON.stringify(e))
      .join("\n") + "\n",
  );
  const source = start(nak, [
    "-q",
    "serve",
    "--hostname",
    "127.0.0.1",
    "--port",
    "17951",
    "--events",
    join(root, "events.jsonl"),
  ]);
  start(nak, ["-q", "serve", "--hostname", "127.0.0.1", "--port", "17952"]);
  start(nak, [
    "-q",
    "serve",
    "--hostname",
    "127.0.0.1",
    "--port",
    "17953",
    "--auth",
  ]);
  await delay(500);
  const secret = randomBytes(16).toString("hex");
  const signer = start(nak, [
    "-q",
    "bunker",
    "--authorized-secrets",
    secret,
    a,
  ]);
  const { NOSTR_SECRET_KEY: fixtureSecret, ...appEnv } = env;
  assert(fixtureSecret);
  start("tauri-driver", ["--port", "4445", "--native-port", "4446"], appEnv);
  await delay(1000);
  assert(
    children.every((p) => p.exitCode === null),
    `A fixture process exited: ${JSON.stringify(children.map((p, index) => ({ index, code: p.exitCode, signal: p.signalCode })))}`,
  );
  assert.equal(
    command(["req", "-k", "1", b]),
    "",
    "Recovery relay must begin empty",
  );
  assert(
    command(["req", "-k", "1", "-a", account, a]).split("\n").filter(Boolean)
      .length >= 12,
    "Independent query positive control failed",
  );
  await open();
  await fill("Password", "Linux recovery fixture password");
  await fill("Confirm password", "Linux recovery fixture password");
  await script(
    "Array.from(document.querySelectorAll('label')).find(l=>l.textContent.includes('I understand')).querySelector('input').click()",
  );
  await click("Create vault");
  await wait("document.body.textContent.includes('Connect Nostr account')");
  await fill(
    "Connection link from your signer (optional)",
    `bunker://${account}?relay=${encodeURIComponent(a)}&secret=${secret}`,
  );
  await fill("Approved signer relay URLs, separated by spaces", a);
  await click("Connect");
  await click("Confirm account");
  await script(
    "Array.from(document.querySelectorAll('details')).find(d=>d.querySelector('summary')?.textContent==='Advanced').open=true",
  );
  await fill("Approved capture relay URLs", a);
  await click("Save backup relay grants");
  await delay(750);
  await fill("Capture source relay", a);
  await click("Run initial collection");
  await wait(
    "(() => { const t = document.querySelector('[aria-labelledby=backup-title]')?.textContent ?? ''; return t.includes('Stored events:') && t.includes('11'); })()",
  );
  report.observations.push(
    "real NIP46 confirmation; 11 eligible kind-1 notes captured into encrypted vault",
  );
  source.kill("SIGTERM");
  signer.kill("SIGTERM");
  await delay(500);
  await click("Check source connection");
  await wait("document.body.textContent.includes('Source relay: OFFLINE')");
  await request(`/session/${session}`, undefined, "DELETE");
  session = undefined;
  await delay(500);
  await open();
  await fill("Password", "Linux recovery fixture password");
  await click("Unlock");
  await wait(
    "(() => { const t = document.querySelector('[aria-labelledby=backup-title]')?.textContent ?? ''; return t.includes('Stored events:') && t.includes('11'); })()",
  );
  assert.equal(
    await script(
      "return document.querySelectorAll('[aria-label=\"Saved kind-1 previews\"] li').length",
    ),
    11,
  );
  report.observations.push(
    "source and signer stopped; fresh application process reads identical retained notes offline",
  );
  await fill("Recovery destination relay", c);
  await script(
    "Array.from(document.querySelectorAll('label')).find(l=>l.textContent.includes('I approve publishing eligible kind-1')).querySelector('input').click()",
  );
  await click("Restore and verify (kind 1 only)");
  await wait(
    "document.querySelector('[aria-labelledby=backup-title] [role=alert]') !== null",
  );
  assert.equal(command(["req", "-k", "1", b]), "");
  report.observations.push(
    "authentication-required destination rejected; no fabricated restore success",
  );
  await fill("Recovery destination relay", b);
  await script(
    "Array.from(document.querySelectorAll('label')).find(l=>l.textContent.includes('I approve publishing eligible kind-1')).querySelector('input').click()",
  );
  await click("Restore and verify (kind 1 only)");
  await wait(
    "document.querySelector('[aria-labelledby=backup-title]')?.textContent.includes('11 of 11')",
  );
  const recovered = command(["req", "-k", "1", "-a", account, b])
    .split("\n")
    .filter(Boolean)
    .map((x) => JSON.parse(x));
  assert.deepEqual(recovered.map((e) => e.id).sort(), expected);
  for (const event of recovered)
    assert.deepEqual(
      event,
      notes.find((n) => n.id === event.id),
      "Signed fields changed",
    );
  const database = await readFile(
    join(root, "com.nostrvault.app/vault/vault.sqlite"),
  );
  for (const marker of [
    "NV_PUBLIC_RECOVERY_FIXTURE_",
    "Linux recovery fixture password",
  ]) {
    assert(
      Buffer.from(`positive-control:${marker}`).includes(Buffer.from(marker)),
    );
    assert(
      !database.includes(Buffer.from(marker)),
      "Plaintext marker in SQLite",
    );
  }
  assert(
    source.exitCode !== null || source.signalCode !== null,
    "Source must remain stopped during verification",
  );
  report.observations.push(
    "fresh nak query of destination returned exact fixture IDs and signed bodies; source remained stopped; SQLite plaintext scan passed",
  );
  report.passed = true;
  console.log(
    "PASS Linux GUI recovery: real signer -> relay A -> encrypted vault -> source/signer stopped -> process restart -> offline notes -> relay B -> independent exact read-back (11/11)",
  );
} finally {
  await mkdir("test-results", { recursive: true });
  await writeFile(
    "test-results/linux-recovery.json",
    JSON.stringify(report, null, 2),
  );
  if (session)
    await request(`/session/${session}`, undefined, "DELETE").catch(() => {});
  for (const child of children) child.kill("SIGTERM");
  await delay(500);
  await rm(root, {
    recursive: true,
    force: true,
    maxRetries: 8,
    retryDelay: 250,
  });
}
