import { createContext, useContext, type ReactNode } from "react";
import type { TenantConfig, UiEntity } from "../sdk/client";
import type { WorkspaceNavItem, WorkspaceShortcut } from "../metadata/types";
import type { Qefro } from "./runtime";

export type QefroSnapshot = {
  entities: UiEntity[];
  config: TenantConfig | null;
  navigation?: string[];
  hiddenEntities?: string[];
  workspaceNav?: WorkspaceNavItem[];
  workspaceShortcuts?: WorkspaceShortcut[];
  userName?: string;
  userEmail?: string;
  roles?: string[];
  studio?: boolean;
};

export type QefroRuntimeValue = {
  runtime: Qefro;
  snapshot: QefroSnapshot;
};

export const QefroRuntimeContext = createContext<QefroRuntimeValue | null>(null);

export function useQefro(): Qefro {
  const value = useContext(QefroRuntimeContext);
  if (!value) {
    throw new Error("useQefro() requires <QefroProvider> (or App wiring that provides the runtime).");
  }
  return value.runtime;
}

export function useQefroOptional(): Qefro | null {
  return useContext(QefroRuntimeContext)?.runtime ?? null;
}

export function useQefroSnapshot(): QefroSnapshot {
  const value = useContext(QefroRuntimeContext);
  return value?.snapshot ?? { entities: [], config: null };
}

export function QefroProvider({
  runtime,
  snapshot,
  children,
}: {
  runtime: Qefro;
  snapshot: QefroSnapshot;
  children: ReactNode;
}) {
  runtime.hydrate({ entities: snapshot.entities, config: snapshot.config });
  return (
    <QefroRuntimeContext.Provider value={{ runtime, snapshot }}>{children}</QefroRuntimeContext.Provider>
  );
}
