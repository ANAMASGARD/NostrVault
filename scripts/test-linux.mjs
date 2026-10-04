import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const profile = await mkdtemp(join(tmpdir(), "nostrvault-smoke-"));
const driver = spawn(
  "tauri-driver",
  ["--port", "4445", "--native-port", "4446"],
  {
    stdio: "inherit",
    env: { ...process.env, XDG_DATA_HOME: profile, XDG_CONFIG_HOME: profile },
  },
);
let driverError;
driver.on("error", (error) => {
  driverError = error;
});
let session;
let teardownFailure;
async function request(path, body, method = "POST") {
  const response = await fetch(`http://127.0.0.1:4445${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(30000),
  });
  const result = await response.json();
  if (!response.ok || result.value?.error)
    throw new Error(JSON.stringify(result));
  return result.value;
}
async function script(source, async = false) {
  return request(`/session/${session}/execute/${async ? "async" : "sync"}`, {
    script: source,
    args: [],
  });
}
try {
  let ready = false;
  for (let attempt = 0; attempt < 40; attempt++) {
    if (driverError) throw driverError;
    try {
      await request("/status", undefined, "GET");
      ready = true;
      break;
    } catch {
      await delay(250);
    }
  }
  assert(ready, "tauri-driver did not become ready");
  const created = await request("/session", {
    capabilities: {
      alwaysMatch: {
        browserName: "wry",
        "tauri:options": {
          application: resolve("src-tauri/target/release/nostrvault"),
        },
      },
    },
  });
  session = created.sessionId;
  assert(session, "No WebDriver session");
  for (let attempt = 0; attempt < 40; attempt++) {
    if (await script('return document.querySelector("button") !== null')) break;
    await delay(250);
  }
  assert.equal(await script("return document.title"), "NostrVault");
  await script('document.querySelector("button").click()');
  let status;
  for (let attempt = 0; attempt < 40; attempt++) {
    status = await script(
      'return document.querySelector("[role=status]").textContent',
    );
    if (status.startsWith("Native runtime ready")) break;
    await delay(250);
  }
  assert.equal(status, "Native runtime ready: linux · 0.1.0");
  const denied = await script(
    `const done = arguments[arguments.length - 1];
    window.__TAURI_INTERNALS__.invoke("plugin:window|set_title", { label: "main", title: "Should be denied" })
      .then(() => done("allowed"), (error) => done(String(error)));`,
    true,
  );
  assert.equal(denied, "Command plugin:window|set_title not allowed by ACL");
  assert.equal(
    await script(
      `const s = document.createElement("script"); s.textContent = "window.__untrustedExecuted = true"; document.body.append(s); return window.__untrustedExecuted === true;`,
    ),
    false,
    "Production CSP must reject inline scripts",
  );
  console.log(
    "PASS Linux packaged release launch, real Rust IPC, denied unapproved IPC, production CSP",
  );
} finally {
  if (session)
    await request(`/session/${session}`, undefined, "DELETE").catch(() => {});
  if (driver.exitCode === null && !driverError) {
    const exited = once(driver, "exit");
    driver.kill("SIGTERM");
    await Promise.race([exited, delay(5000)]);
    if (driver.exitCode === null) driver.kill("SIGKILL");
  }
  for (let attempt = 0; attempt < 30; attempt++) {
    try {
      await rm(profile, {
        recursive: true,
        force: true,
        maxRetries: 3,
        retryDelay: 100,
      });
      break;
    } catch (error) {
      if (error?.code !== "ENOTEMPTY" || attempt === 29) {
        teardownFailure = error;
        break;
      }
      await delay(100);
    }
  }
}
if (teardownFailure) throw teardownFailure;
