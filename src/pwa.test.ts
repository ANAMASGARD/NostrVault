import { expect, it } from "vitest";
import { mayCacheAppShell } from "./pwa";

it("caches the app shell and not vault data", () => {
  expect(mayCacheAppShell("/")).toBe(true);
  expect(mayCacheAppShell("/index.html")).toBe(true);
  expect(mayCacheAppShell("/vault.sqlite")).toBe(false);
  expect(mayCacheAppShell("indexeddb://vault")).toBe(false);
  expect(mayCacheAppShell("/private-data/")).toBe(false);
  expect(mayCacheAppShell("/files/index.html")).toBe(false);
});
