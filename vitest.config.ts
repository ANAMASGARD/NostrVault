import { defineConfig } from "vitest/config";

export default defineConfig({
  define: { __NATIVE_BUILD__: false },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "tests/unit/**/*.test.ts"],
    maxWorkers: 2,
  },
});
