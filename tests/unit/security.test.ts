import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const config = JSON.parse(
  readFileSync("src-tauri/tauri.conf.json", "utf8"),
) as { app: { security: { csp: string } } };
const capability: unknown = JSON.parse(
  readFileSync("src-tauri/capabilities/default.json", "utf8"),
);

describe("foundation security configuration", () => {
  it("limits the local main window to runtime and isolated proof commands", () => {
    expect(capability).toEqual({
      $schema: "../gen/schemas/desktop-schema.json",
      identifier: "default",
      description:
        "Local runtime and isolated foundation proof commands; no plugin access",
      windows: ["main"],
      permissions: ["allow-runtime-info", "allow-foundation-proof"],
    });
  });
  it("requires a production CSP without arbitrary execution or remote connections", () => {
    const directives = config.app.security.csp
      .split(";")
      .map((value) => value.trim());
    expect(directives).toContain("default-src 'self'");
    expect(directives).toContain("script-src 'self'");
    expect(directives).toContain("connect-src ipc: http://ipc.localhost");
    expect(directives).toContain("object-src 'none'");
    expect(config.app.security.csp).not.toMatch(
      /unsafe-inline|unsafe-eval|https:|\*/,
    );
  });
});
