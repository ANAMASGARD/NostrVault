import { expect, test, chromium, firefox } from "@playwright/test";
import { execFileSync } from "node:child_process";
import {
  readFileSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
const secret = "public test passphrase";
async function create(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByLabel("Password", { exact: true }).fill(secret);
  await page.getByLabel("Confirm password", { exact: true }).fill(secret);
  await page.getByRole("checkbox", { name: /I understand/ }).check();
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(
    page.getByText("Setup saved securely on this device."),
  ).toBeVisible({ timeout: 30000 });
}
test("real vault survives restart and password change without rewriting encrypted setup", async ({
  page,
  context,
}) => {
  test.setTimeout(120000);
  const external: string[] = [];
  context.on("request", (r) => {
    if (new URL(r.url()).origin !== "http://127.0.0.1:1420")
      external.push(r.url());
  });
  await create(page);
  const inspect = () =>
    page.evaluate(async () => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const r = indexedDB.open("nostrvault-vault");
        r.onsuccess = () => resolve(r.result);
        r.onerror = () => reject(new Error("open"));
      });
      const result = await new Promise<string>((resolve, reject) => {
        const tx = db.transaction(["metadata", "records"]);
        const records = tx.objectStore("records").getAll();
        const metadata = tx.objectStore("metadata").getAll();
        tx.oncomplete = () =>
          resolve(
            JSON.stringify({
              records: records.result as unknown,
              metadata: metadata.result as unknown,
            }),
          );
        tx.onabort = () => reject(new Error("abort"));
      });
      db.close();
      return result;
    });
  const before = await inspect();
  expect(before).not.toContain(secret);
  expect(before).not.toContain("lossAcknowledged");
  // Positive controls demonstrate that the logical-store scanner finds plaintext.
  expect(JSON.stringify({ password: secret })).toContain(secret);
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
  await page
    .getByLabel("Password", { exact: true })
    .fill("incorrect test password");
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "could not be authenticated",
    { timeout: 30000 },
  );
  await page.getByLabel("Password", { exact: true }).fill(secret);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(
    page.getByText("Setup saved securely on this device."),
  ).toBeVisible({ timeout: 30000 });
  await page
    .getByRole("button", { name: "Change password", exact: true })
    .click();
  await page.getByLabel("Current password", { exact: true }).fill(secret);
  await page
    .getByLabel("New password", { exact: true })
    .fill("replacement test password");
  await page
    .getByLabel("Confirm password", { exact: true })
    .fill("replacement test password");
  await page
    .getByRole("button", { name: "Change password", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible({ timeout: 30000 });
  const after = await inspect();
  const original: unknown = JSON.parse(before);
  const replacement: unknown = JSON.parse(after);
  expect(
    typeof original === "object" && original !== null && "records" in original
      ? original.records
      : null,
  ).toEqual(
    typeof replacement === "object" &&
      replacement !== null &&
      "records" in replacement
      ? replacement.records
      : null,
  );
  await page.reload();
  await page.getByLabel("Password", { exact: true }).fill(secret);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "could not be authenticated",
    { timeout: 30000 },
  );
  await page
    .getByLabel("Password", { exact: true })
    .fill("replacement test password");
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(
    page.getByText("Setup saved securely on this device."),
  ).toBeVisible({ timeout: 30000 });
  await page.getByRole("button", { name: "Lock now" }).click();
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
  expect(external).toEqual([]);
});
test("competing tabs cannot own an unlocked vault and owner exit releases it", async ({
  page,
  context,
}) => {
  test.setTimeout(60000);
  await create(page);
  const second = await context.newPage();
  await second.goto("/");
  await expect(second.getByRole("alert")).toContainText("in use");
  await page.close();
  await second.getByRole("button", { name: "Retry storage" }).click();
  await expect(
    second.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
});
test("lock cancels derivation and late results cannot reveal setup", async ({
  page,
}) => {
  test.setTimeout(60000);
  await create(page);
  await page.getByRole("button", { name: "Lock now" }).click();
  await page.getByLabel("Password", { exact: true }).fill(secret);
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await page.getByRole("button", { name: "Cancel and lock" }).click();
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Setup saved securely on this device."),
  ).toHaveCount(0);
  await expect(page.getByLabel("Password", { exact: true })).toHaveValue("");
});
test("production KDF isolated Worker measurements", async ({
  page,
  context,
}, testInfo) => {
  test.setTimeout(120000);
  await context.route("**/__vault/*", async (route) => {
    const name = new URL(route.request().url()).pathname.split("/").pop();
    if (name === "vault_wasm.js" || name === "vault_wasm_bg.wasm")
      await route.fulfill({
        contentType: name.endsWith(".wasm")
          ? "application/wasm"
          : "text/javascript",
        body: readFileSync(`src/generated/${name}`),
      });
    else await route.abort();
  });
  await page.goto("/?diagnostics");
  const measurement = await page.evaluate(
    () =>
      new Promise<{ milliseconds: number[]; vector: string }>(
        (resolve, reject) => {
          const source = `import init,{vault_kdf_probe,vault_record_vectors} from '${location.origin}/__vault/vault_wasm.js';await init({module_or_path:'${location.origin}/__vault/vault_wasm_bg.wasm'});const vector=vault_record_vectors();if(vector.includes('M03 PRIVATE MESSAGE MARKER'))throw new Error('plaintext');let times=[];for(let i=0;i<6;i++){let start=performance.now();vault_kdf_probe();if(i)times.push(performance.now()-start);}postMessage({milliseconds:times,vector});`;
          const url = URL.createObjectURL(
            new Blob([source], { type: "text/javascript" }),
          );
          const worker = new Worker(url, { type: "module" });
          worker.onerror = () => {
            worker.terminate();
            URL.revokeObjectURL(url);
            reject(new Error("probe failed"));
          };
          worker.onmessage = (e: MessageEvent<unknown>) => {
            worker.terminate();
            URL.revokeObjectURL(url);
            if (
              typeof e.data === "object" &&
              e.data !== null &&
              "milliseconds" in e.data &&
              Array.isArray(e.data.milliseconds) &&
              e.data.milliseconds.every(
                (n: unknown) => typeof n === "number",
              ) &&
              "vector" in e.data &&
              typeof e.data.vector === "string"
            )
              resolve({
                milliseconds: e.data.milliseconds,
                vector: e.data.vector,
              });
            else reject(new Error("malformed"));
          };
        },
      ),
  );
  const { milliseconds, vector } = measurement;
  execFileSync(
    "cargo",
    [
      "run",
      "--locked",
      "--release",
      "-p",
      "vault-native",
      "--example",
      "vault_exchange",
    ],
    { input: vector, encoding: "utf8" },
  );
  expect(milliseconds).toHaveLength(5);
  expect(milliseconds.every((n) => n > 0)).toBe(true);
  const report = JSON.stringify({
    profile: "65536KiB/t3/p4",
    browser: context.browser()?.version(),
    milliseconds,
    memory: "64 MiB Argon2 workspace; browser RSS not isolated",
  });
  writeFileSync(`test-results/m03-${testInfo.project.name}-kdf.json`, report);
  await testInfo.attach("kdf-measurements", {
    body: report,
    contentType: "application/json",
  });
});

test("real IndexedDB rolls back batches and interrupted encrypted migration", async ({
  page,
  context,
}) => {
  test.setTimeout(60000);
  // Serve the actual adapter source transpiled for this isolated test; no mock DB.
  const ts = await import("typescript");
  await context.route("**/__storage/*", async (route) => {
    const name = new URL(route.request().url()).pathname.split("/").pop();
    if (name !== "vault-store.js" && name !== "vault-contract.js") {
      await route.abort();
      return;
    }
    const source = readFileSync(
      `src/${name.replace(".js", ".ts")}`,
      "utf8",
    ).replace('"./vault-contract"', '"./vault-contract.js"');
    await route.fulfill({
      contentType: "text/javascript",
      body: ts.transpileModule(source, {
        compilerOptions: {
          target: ts.ScriptTarget.ES2020,
          module: ts.ModuleKind.ES2020,
        },
      }).outputText,
    });
  });
  await create(page);
  await page.getByRole("button", { name: "Lock now" }).click();
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
  const result = await page.evaluate(async () => {
    // Variable import is resolved only by Playwright's test route.
    const url = location.origin + "/__storage/vault-store.js";
    const adapter: unknown = await import(/* @vite-ignore */ url);
    if (typeof adapter !== "object" || adapter === null)
      throw new Error("module");
    // Run the test harness as browser code so all calls use the real IDB module.
    return await new Promise<Record<string, boolean>>((resolve, reject) => {
      const source = `import * as store from '${url}';
      try {
        const db=await store.openVaultStore();
        const snapshot=await store.readSnapshot(db);
        const all=await new Promise((resolve,reject)=>{const tx=db.transaction('records');const r=tx.objectStore('records').getAll();tx.oncomplete=()=>resolve(r.result);tx.onabort=()=>reject('abort');});
        const original=all[0];
        const replacement={...original,revision:original.revision+1};
        let rollback=false;try{await store.commitVault(db,{expectedRevision:snapshot.revision,header:null,records:[replacement],create:false},0);}catch(e){rollback=e.message==='quota';}
        rollback=rollback && (await store.readSnapshot(db)).revision===snapshot.revision && JSON.stringify(await store.readRecord(db,original.key))===JSON.stringify(original);
        await store.stageMigration(db,snapshot.revision,[replacement],true);
        let interrupted=false;try{await store.finalizeMigration(db,snapshot.revision,true);}catch(e){interrupted=e.message==='quota';}
        interrupted=interrupted && JSON.stringify(await store.readRecord(db,original.key))===JSON.stringify(original);
        db.close();const reopened=await store.openVaultStore();
        await store.finalizeMigration(reopened,snapshot.revision);
        const committed=(await store.readSnapshot(reopened)).revision===snapshot.revision+1;
        const isolation=(await store.pageRecords(reopened,'f'.repeat(64),'',16)).length===0;
        const page=(await store.pageRecords(reopened,original.namespace,'',16)).length===1;
        await store.deleteRecords(reopened,snapshot.revision+1,[original.key]);
        const deleted=(await store.pageRecords(reopened,original.namespace,'',16)).length===0;
        reopened.close();postMessage({rollback,interrupted,committed,isolation,page,deleted});
      } catch(e){postMessage({error:String(e)});}`;
      const blob = URL.createObjectURL(
        new Blob([source], { type: "text/javascript" }),
      );
      const worker = new Worker(blob, { type: "module" });
      worker.onerror = () => {
        worker.terminate();
        URL.revokeObjectURL(blob);
        reject(new Error("storage harness failed"));
      };
      worker.onmessage = (event: MessageEvent<unknown>) => {
        worker.terminate();
        URL.revokeObjectURL(blob);
        const v = event.data;
        if (typeof v !== "object" || v === null || "error" in v) {
          reject(new Error("storage harness failed"));
          return;
        }
        const result: Record<string, boolean> = {};
        for (const [key, value] of Object.entries(v)) {
          if (typeof value !== "boolean") {
            reject(new Error("invalid result"));
            return;
          }
          result[key] = value;
        }
        resolve(result);
      };
    });
  });
  expect(result).toEqual({
    rollback: true,
    interrupted: true,
    committed: true,
    isolation: true,
    page: true,
    deleted: true,
  });
});

test("browser process restart retains encrypted vault and backing artifacts exclude markers", async ({
  browserName,
}) => {
  test.setTimeout(90000);
  const directory = mkdtempSync(join(tmpdir(), "nostrvault-browser-m03-"));
  const type = browserName === "firefox" ? firefox : chromium;
  let context = await type.launchPersistentContext(directory, {
    headless: true,
    baseURL: "http://127.0.0.1:1420",
  });
  try {
    let page = await context.newPage();
    await create(page);
    await context.close();
    context = await type.launchPersistentContext(directory, {
      headless: true,
      baseURL: "http://127.0.0.1:1420",
    });
    page = await context.newPage();
    await page.goto("/");
    await expect(
      page.getByRole("button", { name: "Unlock", exact: true }),
    ).toBeVisible();
    await page.getByLabel("Password", { exact: true }).fill(secret);
    await page.getByRole("button", { name: "Unlock", exact: true }).click();
    await expect(
      page.getByText("Setup saved securely on this device."),
    ).toBeVisible({ timeout: 30000 });
    await context.close();
    const files: string[] = [];
    function scan(path: string) {
      for (const entry of readdirSync(path, { withFileTypes: true })) {
        const full = join(path, entry.name);
        if (entry.isDirectory()) scan(full);
        else if (entry.isFile() && /IndexedDB|storage\/default/.test(full))
          files.push(full);
      }
    }
    scan(directory);
    expect(files.length).toBeGreaterThan(0);
    const markers = [
      secret,
      "replacement test password",
      "M03 PRIVATE MESSAGE MARKER",
    ];
    const detect = (bytes: Buffer) =>
      markers.some((marker) => bytes.includes(Buffer.from(marker)));
    const control = join(directory, "plaintext-control");
    writeFileSync(control, markers.join("\n"));
    expect(detect(readFileSync(control))).toBe(true);
    for (const file of files)
      expect(detect(readFileSync(file)), file).toBe(false);
  } finally {
    await context.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("newer browser schema is refused without modifying its data", async ({
  page,
}) => {
  await page.goto("/?diagnostics");
  await page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open("nostrvault-vault", 99);
      r.onupgradeneeded = () => r.result.createObjectStore("sentinel");
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(new Error("open"));
    });
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction("sentinel", "readwrite");
      tx.objectStore("sentinel").put("preserve future vault", "marker");
      tx.oncomplete = () => resolve();
      tx.onabort = () => reject(new Error("abort"));
    });
    db.close();
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("not supported");
  const result = await page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open("nostrvault-vault");
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(new Error("open"));
    });
    const value = await new Promise<unknown>((resolve, reject) => {
      const tx = db.transaction("sentinel");
      const r = tx.objectStore("sentinel").get("marker");
      tx.oncomplete = () => resolve(r.result);
      tx.onabort = () => reject(new Error("abort"));
    });
    const version = db.version;
    db.close();
    return { version, value };
  });
  expect(result).toEqual({ version: 99, value: "preserve future vault" });
});

test("blocked earlier production schema upgrade retries after the old connection closes", async ({
  page,
  context,
}) => {
  const holder = await context.newPage();
  await holder.goto("/?diagnostics");
  await holder.evaluate(async () => {
    // Explicit earlier production-schema fixture; never the M02 proof database.
    await new Promise<void>((resolve, reject) => {
      const r = indexedDB.open("nostrvault-vault", 1);
      r.onupgradeneeded = () => {
        r.result.createObjectStore("metadata");
        r.result.createObjectStore("records");
      };
      r.onsuccess = () => {
        r.result.onversionchange = () => {};
        resolve();
      };
      r.onerror = () => reject(new Error("fixture open"));
    });
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("another");
  await holder.close();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Create vault", exact: true }),
  ).toBeVisible();
});

test("future vault format aborts an earlier storage schema upgrade", async ({
  page,
}) => {
  await page.goto("/?diagnostics");
  await page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open("nostrvault-vault", 1);
      r.onupgradeneeded = () => {
        r.result.createObjectStore("metadata");
        r.result.createObjectStore("records");
      };
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(new Error("fixture"));
    });
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction("metadata", "readwrite");
      tx.objectStore("metadata").put(
        { header: { format: 99, wrapper: 1 }, revision: 1 },
        "vault",
      );
      tx.oncomplete = () => resolve();
      tx.onabort = () => reject(new Error("fixture"));
    });
    db.close();
  });
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("not supported");
  const version = await page.evaluate(
    async () =>
      await new Promise<number>((resolve, reject) => {
        const r = indexedDB.open("nostrvault-vault");
        r.onsuccess = () => {
          const version = r.result.version;
          r.result.close();
          resolve(version);
        };
        r.onerror = () => reject(new Error("inspect"));
      }),
  );
  expect(version).toBe(1);
});
