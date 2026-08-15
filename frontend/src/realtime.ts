import { useEffect, useRef } from "react";
import { tokenHeader } from "./api";

export function useRealtime(
  opts: { entity?: string; recordId?: string; enabled?: boolean },
  onEvent: () => void,
) {
  const handler = useRef(onEvent);
  handler.current = onEvent;

  useEffect(() => {
    if (opts.enabled === false) return;
    const ac = new AbortController();
    const params = new URLSearchParams();
    if (opts.entity) params.set("entity", opts.entity);
    if (opts.recordId) params.set("record_id", opts.recordId);
    (async () => {
      const res = await fetch(`/api/v1/realtime?${params}`, {
        headers: tokenHeader(),
        signal: ac.signal,
      });
      if (!res.ok || !res.body) return;
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
          if (part.split("\n").some((line) => line.startsWith("data:"))) {
            handler.current();
          }
        }
      }
    })().catch(() => undefined);
    return () => ac.abort();
  }, [opts.entity, opts.recordId, opts.enabled]);
}
