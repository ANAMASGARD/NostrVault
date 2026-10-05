import { expect, test } from "@playwright/test";
const account =
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
async function vault(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill("M04 disposable password");
  await page
    .getByLabel("Confirm password", { exact: true })
    .fill("M04 disposable password");
  await page.getByRole("checkbox", { name: /I understand/ }).check();
  await page.getByRole("button", { name: "Create vault", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Connect", exact: true }),
  ).toBeEnabled({ timeout: 30000 });
}
test("missing signer fails without identity secret fallback", async ({
  page,
}) => {
  await vault(page);
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("No compatible signer");
  await expect(page.locator('input[type="password"]')).toHaveCount(0);
});
test("explicit confirmation, missing optional methods, encrypted remembered account and disconnect", async ({
  page,
}) => {
  await page.addInitScript((a) => {
    Object.defineProperty(window, "nostr", {
      value: { getPublicKey: () => Promise.resolve(a) },
    });
  }, account);
  await vault(page);
  await page
    .getByLabel("Remember this account and local consent choices")
    .check();
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByText("Confirm this is the account")).toBeVisible();
  await expect(
    page.getByText("Account connected", { exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Confirm account" }).click();
  await expect(
    page.getByText("Account connected", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel(/Readable private chats:/)).toBeDisabled();
  const persisted = await page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open("nostrvault-vault");
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(new Error("open"));
    });
    const result = await new Promise<string>((resolve) => {
      const tx = db.transaction("records");
      const r = tx.objectStore("records").getAll();
      tx.oncomplete = () => resolve(JSON.stringify(r.result));
    });
    db.close();
    return result;
  });
  expect(persisted).not.toContain(account);
  expect(persisted).not.toContain("readable");
  await page.getByRole("button", { name: "Lock now" }).click();
  await page
    .getByLabel("Password", { exact: true })
    .fill("M04 disposable password");
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Retry connection" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Retry connection" }).click();
  await expect(
    page.getByText("Account connected", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Disconnect and remove local signer access" })
    .click();
  await expect(
    page.getByRole("button", { name: "Connect", exact: true }),
  ).toBeEnabled();
  await page.reload();
  await page
    .getByLabel("Password", { exact: true })
    .fill("M04 disposable password");
  await page.getByRole("button", { name: "Unlock", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Connect", exact: true }),
  ).toBeEnabled();
  await expect(page.getByText(account, { exact: true })).toHaveCount(0);
});
test("lock discards late browser approval", async ({ page }) => {
  await page.addInitScript((a) => {
    Object.defineProperty(window, "nostr", {
      value: {
        getPublicKey: () =>
          new Promise((resolve) => {
            Reflect.set(window, "fixtureApprove", () => resolve(a));
          }),
      },
    });
  }, account);
  await vault(page);
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(
    page.getByText("Waiting for approval", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Lock now" }).click();
  await page.evaluate(() => {
    const fn: unknown = Reflect.get(window, "fixtureApprove");
    if (typeof fn === "function") Reflect.apply(fn, window, []);
  });
  await expect(
    page.getByRole("button", { name: "Unlock", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Confirm account" }),
  ).toHaveCount(0);
});
test("denial never retries automatically", async ({ page }) => {
  await page.addInitScript(() => {
    Reflect.set(window, "fixtureCalls", 0);
    Object.defineProperty(window, "nostr", {
      value: {
        getPublicKey: () => {
          Reflect.set(
            window,
            "fixtureCalls",
            Number(Reflect.get(window, "fixtureCalls")) + 1,
          );
          return Promise.reject(
            Object.assign(new Error("TEST denial"), { code: 4001 }),
          );
        },
      },
    });
  });
  await vault(page);
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Approval was declined");
  expect(
    await page.evaluate(() => Number(Reflect.get(window, "fixtureCalls"))),
  ).toBe(1);
});
