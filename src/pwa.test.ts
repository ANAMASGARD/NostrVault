import { expect, it } from "vitest";
import { mayCacheAppShell } from "./pwa";

it("caches the app shell and not vault data", () => {
  expect(mayCacheAppShell("/index.html")).toBe(true);
  expect(mayCacheAppShell("/vault.sqlite")).toBe(false);
  expect(mayCacheAppShell("indexeddb://vault")).toBe(false);
});
