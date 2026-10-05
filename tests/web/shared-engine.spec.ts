import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

test("shared engine persists encrypted fixture across actual Worker replacement", async ({
  page,
}) => {
  let workers = 0;
  page.on("worker", () => workers++);
  await page.goto("/?diagnostics");
  await page.getByRole("button", { name: "Check shared engine" }).click();
  await expect(page.getByTestId("foundation-result")).toHaveText(
    /Rust\/WASM foundation ready · \d+ fixture bytes · encrypted storage reopened/,
    { timeout: 60000 },
  );
  expect(workers).toBe(2);
  await page.reload();
  await page.getByRole("button", { name: "Check shared engine" }).click();
  await expect(page.getByTestId("foundation-result")).toHaveText(
    /encrypted storage reopened/,
    { timeout: 60000 },
  );
  expect(workers).toBe(4);
});

test("cancels a pending Worker and a fresh request succeeds", async ({
  page,
  context,
}) => {
  await context.route("**/*vault_wasm*.wasm", () => {});
  await page.goto("/?diagnostics");
  await page.getByRole("button", { name: "Check shared engine" }).click();
  await page.getByRole("button", { name: "Cancel engine check" }).click();
  await expect(page.getByTestId("foundation-result")).toHaveText(
    "Foundation check failed or cancelled. Try again.",
  );
  await context.unroute("**/*vault_wasm*.wasm");
  await page.getByRole("button", { name: "Check shared engine" }).click();
  await expect(page.getByTestId("foundation-result")).toHaveText(
    /encrypted storage reopened/,
    { timeout: 60000 },
  );
});

// Exercise compiled exports in an actual browser Worker. Only public test fixtures
// cross this harness; application transport remains transferable binary buffers.
test("WASM rejects invalid data and exchanges age ciphertext with native and reference implementations", async ({
  page,
  context,
}) => {
  test.setTimeout(120000);
  const dir = mkdtempSync(join(tmpdir(), "nostrvault-exchange-"));
  try {
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "-p",
        "vault-native",
        "--example",
        "age_exchange",
        "--",
        "write",
        dir,
      ],
      { stdio: "pipe" },
    );
    await context.route("**/__foundation/*", async (route) => {
      const name = new URL(route.request().url()).pathname.split("/").pop();
      if (name === "vault_wasm.js" || name === "vault_wasm_bg.wasm") {
        await route.fulfill({
          contentType: name.endsWith(".wasm")
            ? "application/wasm"
            : "text/javascript",
          body: readFileSync(`src/generated/${name}`),
        });
      } else await route.abort();
    });
    await page.goto("/?diagnostics");
    const fixtures = {
      event: readFileSync("crates/vault-core/fixtures/event.json", "utf8"),
      payload: Array.from(
        readFileSync("crates/vault-core/fixtures/payload.zip"),
      ),
      recipient: readFileSync(
        "crates/shared/fixtures/age-recipient.txt",
        "utf8",
      ).trim(),
      identity: readFileSync(
        "crates/shared/fixtures/age-identity.txt",
        "utf8",
      ).trim(),
      passphrase: Array.from(
        readFileSync("crates/shared/fixtures/reference-passphrase.age"),
      ),
      recipientCiphertext: Array.from(
        readFileSync("crates/shared/fixtures/reference-recipient.age"),
      ),
      nativePassphrase: Array.from(
        readFileSync(join(dir, "native-passphrase.age")),
      ),
      nativeRecipient: Array.from(
        readFileSync(join(dir, "native-recipient.age")),
      ),
    };
    const result = await page.evaluate(async (fixtures) => {
      const code = `
        import init, * as wasm from "${location.origin}/__foundation/vault_wasm.js";
        onmessage = async ({data: f}) => {
          try {
            await init({module_or_path: "${location.origin}/__foundation/vault_wasm_bg.wasm"});
            const payload = new Uint8Array(f.payload);
            const event = new TextEncoder().encode(f.event);
            const request = {version:1,requestId:"browser-proof",operation:"foundation_proof",eventLength:event.length,payloadLength:payload.length};
            const equal = bytes => bytes.length === payload.length && bytes.every((v,i)=>v===payload[i]);
            const mustReject = fn => { let rejected=false; try { fn(); } catch { rejected=true; } if(!rejected) throw Error("expected rejection"); };
            for(const update of [{version:2},{operation:"unknown"},{eventLength:1},{requestId:""}]) mustReject(()=>wasm.protect(JSON.stringify({...request,...update}),event,payload));
            mustReject(()=>wasm.protect("{}",event,payload));
            mustReject(()=>wasm.protect(JSON.stringify(request),new Uint8Array(65537),payload));
            mustReject(()=>wasm.protect(JSON.stringify(request),event,new Uint8Array(1048577)));
            for(const key of ["content","created_at","tags","id","sig"]) {
              const changed=JSON.parse(f.event);
              if(key==="content") changed.content+="changed";
              else if(key==="created_at") changed.created_at++;
              else if(key==="tags") changed.tags.push(["changed"]);
              else changed[key]="0".repeat(changed[key].length);
              mustReject(()=>wasm.validated_event_id(new TextEncoder().encode(JSON.stringify(changed))));
            }
            const encrypted=wasm.protect(JSON.stringify(request),event,payload);
            if(!equal(wasm.recover(event,encrypted))) throw Error("record roundtrip");
            const tampered=encrypted.slice(); tampered[tampered.length-4]^=1;
            mustReject(()=>wasm.recover(event,tampered));
            for(const data of [f.passphrase,f.nativePassphrase]) if(!equal(wasm.age_decrypt(new Uint8Array(data)))) throw Error("passphrase interoperability");
            for(const data of [f.recipientCiphertext,f.nativeRecipient]) if(!equal(wasm.age_decrypt_recipient(new Uint8Array(data),f.identity))) throw Error("recipient interoperability");
            const pass=wasm.age_encrypt(payload), recipient=wasm.age_encrypt_recipient(payload,f.recipient);
            if(!equal(wasm.age_decrypt(pass)) || !equal(wasm.age_decrypt_recipient(recipient,f.identity))) throw Error("WASM age roundtrip");
            for(const data of [pass.slice(0,-1),new Uint8Array([1,2,3])]) mustReject(()=>wasm.age_decrypt(data));
            const originalRandom = crypto.getRandomValues;
            crypto.getRandomValues = () => { throw Error("test entropy unavailable"); };
            try { mustReject(()=>wasm.protect(JSON.stringify(request),event,payload)); }
            finally { crypto.getRandomValues = originalRandom; }
            postMessage({pass:Array.from(pass),recipient:Array.from(recipient)});
          } catch { postMessage({error:"WASM proof failed"}); }
        };
      `;
      const url = URL.createObjectURL(
        new Blob([code], { type: "text/javascript" }),
      );
      const worker = new Worker(url, { type: "module" });
      try {
        return await new Promise<{ pass: number[]; recipient: number[] }>(
          (resolve, reject) => {
            worker.onerror = () =>
              reject(new Error("Worker fixture proof failed"));
            worker.onmessage = (
              event: MessageEvent<{
                pass: number[];
                recipient: number[];
                error?: string;
              }>,
            ) =>
              event.data.error
                ? reject(new Error(event.data.error))
                : resolve(event.data);
            worker.postMessage(fixtures);
          },
        );
      } finally {
        worker.terminate();
        URL.revokeObjectURL(url);
      }
    }, fixtures);
    writeFileSync(join(dir, "wasm-passphrase.age"), Buffer.from(result.pass));
    writeFileSync(
      join(dir, "wasm-recipient.age"),
      Buffer.from(result.recipient),
    );
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "-p",
        "vault-native",
        "--example",
        "age_exchange",
        "--",
        "verify",
        dir,
      ],
      { stdio: "pipe" },
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
