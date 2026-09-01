export type UiEventMap = {
  "entity:created": { entity: string; slug: string; id: string };
  "entity:updated": { entity: string; slug: string; id: string };
  "entity:deleted": { entity: string; slug: string; id: string };
  "route:change": { pathname: string; search: string };
  "workspace:ready": { entities: number };
};

export type UiEventName = keyof UiEventMap;
export type UiEventHandler<K extends UiEventName = UiEventName> = (payload: UiEventMap[K]) => void;

type AnyHandler = (payload: unknown) => void;

const listeners = new Map<string, Set<AnyHandler>>();

export function onUiEvent<K extends UiEventName>(event: K, handler: UiEventHandler<K>): () => void {
  const set = listeners.get(event) ?? new Set<AnyHandler>();
  set.add(handler as AnyHandler);
  listeners.set(event, set);
  return () => {
    set.delete(handler as AnyHandler);
    if (set.size === 0) listeners.delete(event);
  };
}

export function emitUiEvent<K extends UiEventName>(event: K, payload: UiEventMap[K]): void {
  const set = listeners.get(event);
  if (!set) return;
  for (const handler of set) handler(payload);
}

export function resetUiEvents(): void {
  listeners.clear();
}
