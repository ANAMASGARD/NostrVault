import { checkFoundation } from "./foundation";

export type NativeRuntime = {
  platform: "linux" | "android";
  version: string;
};

export function parseNativeRuntime(value: unknown): NativeRuntime {
  if (
    typeof value !== "object" ||
    value === null ||
    !("platform" in value) ||
    (value.platform !== "linux" && value.platform !== "android") ||
    !("version" in value) ||
    typeof value.version !== "string" ||
    !/^\d+\.\d+\.\d+$/.test(value.version)
  ) {
    throw new Error("Invalid native runtime response");
  }
  return { platform: value.platform, version: value.version };
}

export async function checkRuntime(): Promise<string> {
  // Vite removes this branch and its IPC dependency from the web build.
  if (__NATIVE_BUILD__) {
    const { invoke } = await import("@tauri-apps/api/core");
    const result = parseNativeRuntime(await invoke<unknown>("runtime_info"));
    return `Native runtime ready: ${result.platform} · ${result.version}`;
  }
  return checkFoundation();
}
