import { object } from "./vault-contract";
import type { Effect } from "./identity-contract";
export async function remoteSocket(
  effect: Effect,
  signal: AbortSignal,
): Promise<unknown> {
  for (const relay of effect.relays) {
    if (signal.aborted) throw new Error("cancelled");
    try {
      return await new Promise((resolve, reject) => {
        const socket = new WebSocket(relay);
        let opened = false;
        const finish = (value?: unknown, error?: string) => {
          clearTimeout(timer);
          signal.removeEventListener("abort", abort);
          socket.onmessage = null;
          socket.onerror = null;
          socket.onclose = null;
          socket.close();
          if (error) reject(new Error(error));
          else resolve(value);
        };
        const abort = () => finish(undefined, "cancelled");
        signal.addEventListener("abort", abort, { once: true });
        const timer = setTimeout(
          () => finish(undefined, "timeout"),
          Math.max(1, effect.deadline * 1000 - Date.now()),
        );
        socket.onopen = () => {
          opened = true;
          for (const message of effect.messages)
            socket.send(JSON.stringify(message));
        };
        socket.onerror = () => finish(undefined, "unavailable");
        socket.onclose = () =>
          finish(undefined, opened ? "unavailable" : "missing_signer");
        socket.onmessage = (event: MessageEvent<unknown>) => {
          if (typeof event.data !== "string" || event.data.length > 262144) {
            finish(undefined, "malformed");
            return;
          }
          try {
            const v: unknown = JSON.parse(event.data);
            if (
              Array.isArray(v) &&
              v.length === 3 &&
              v[0] === "EVENT" &&
              v[1] === effect.id &&
              object(v[2])
            )
              finish(v[2]);
          } catch {
            finish(undefined, "malformed");
          }
        };
      });
    } catch (error) {
      if (signal.aborted) throw error;
    }
  }
  throw new Error("unavailable");
}
