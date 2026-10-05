import { expect, test } from "@playwright/test";

test("loads the production web app, checks runtime, and reloads without native IPC", async ({
  page,
  context,
}) => {
  let wasmRequests = 0;
  let workers = 0;
  context.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith(".wasm")) wasmRequests++;
  });
  page.on("worker", () => workers++);
  const failures: string[] = [];
  const external: string[] = [];
  page.on("pageerror", (error) => failures.push(error.message));
  context.on("request", (request) => {
    if (new URL(request.url()).origin !== "http://127.0.0.1:1420")
      external.push(request.url());
  });
  await page.goto("/?diagnostics");
  await expect(
    page.getByRole("heading", { name: "NostrVault", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("No account or relay is contacted.", { exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Check runtime" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("status")).toHaveText(
    /Rust\/WASM foundation ready · \d+ fixture bytes · encrypted storage reopened/,
    { timeout: 60000 },
  );
  expect(wasmRequests).toBe(2);
  expect(workers).toBe(2);
  await page.reload();
  await expect(page.getByRole("status")).toHaveText("Not checked");
  expect(await page.evaluate(() => "__TAURI_INTERNALS__" in window)).toBe(
    false,
  );
  expect(failures).toEqual([]);
  expect(external).toEqual([]);
});

test("fits a narrow mobile viewport without horizontal overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 740 });
  await page.goto("/?diagnostics");
  await expect(
    page.getByRole("button", { name: "Check runtime" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});

for (const failure of ["missing", "corrupt", "unloadable"] as const) {
  test(`fails explicitly when WASM is ${failure}, then recovers on retry`, async ({
    page,
    context,
  }) => {
    let intercepted = 0;
    await context.route("**/*.wasm", async (route) => {
      intercepted++;
      if (failure === "unloadable") await route.abort();
      else
        await route.fulfill({
          status: failure === "missing" ? 404 : 200,
          contentType: "application/wasm",
          body: "not a wasm module",
        });
    });
    await page.goto("/?diagnostics");
    await page.getByRole("button", { name: "Check shared engine" }).click();
    await expect(page.getByTestId("foundation-result")).toHaveText(
      "Foundation check failed or cancelled. Try again.",
    );
    expect(intercepted).toBe(1);
    await expect(
      page.getByRole("button", { name: "Check shared engine" }),
    ).toBeEnabled();
    await context.unroute("**/*.wasm");
    await page.getByRole("button", { name: "Check shared engine" }).click();
    await expect(page.getByTestId("foundation-result")).toHaveText(
      /Rust\/WASM foundation ready · \d+ fixture bytes · encrypted storage reopened/,
      { timeout: 60000 },
    );
  });
}
