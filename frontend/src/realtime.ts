import { useEffect, useRef, useState } from "react";
import { tokenHeader } from "./api";

export type RealtimeStatus = "connecting" | "open" | "closed";

export function useRealtime(
  opts: { entity?: string; recordId?: string; enabled?: boolean },
  onEvent: () => void,
): { connected: boolean; status: RealtimeStatus } {
  const handler = useRef(onEvent);
  handler.current = onEvent;
  const [status, setStatus] = useState<RealtimeStatus>("closed");

  useEffect(() => {
    if (opts.enabled === false) return;
    const ac = new AbortController();
    let attempt = 0;
    let timer: number | undefined;

    async function connect() {
      if (ac.signal.aborted) return;
      setStatus(attempt === 0 ? "connecting" : "connecting");
      const params = new URLSearchParams();
      if (opts.entity) params.set("entity", opts.entity);
      if (opts.recordId) params.set("record_id", opts.recordId);
      try {
        const res = await fetch(`/api/v1/realtime?${params}`, {
          headers: tokenHeader(),
          signal: ac.signal,
        });
        if (!res.ok || !res.body) throw new Error("realtime unavailable");
        setStatus("open");
        attempt = 0;
        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buf = "";
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buf += decoder.decode(value, { stream: true });
          const parts = buf.split("\n\n");
          buf = parts.pop() ?? "";
          for (const part of parts) {
            const lines = part.split("\n");
            if (lines.every((line) => line.startsWith(":") || line.trim() === "")) continue;
            if (lines.some((line) => line.startsWith("data:"))) handler.current();
          }
        }
        setStatus("closed");
      } catch {
        setStatus("closed");
      }
      if (ac.signal.aborted) return;
      const delay = Math.min(30_000, 1000 * 2 ** Math.min(attempt, 5));
      attempt += 1;
      timer = window.setTimeout(() => void connect(), delay);
    }

    void connect();
    return () => {
      ac.abort();
      if (timer) window.clearTimeout(timer);
    };
  }, [opts.entity, opts.recordId, opts.enabled]);

  return { connected: status === "open", status };
}
