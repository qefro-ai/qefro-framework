import { createElement, type ReactNode } from "react";
import type { UiEntity, ViewKind } from "../metadata/types";

export type CollectionViewProps = {
  meta: UiEntity;
  entities: UiEntity[];
  slug: string;
  rows: Record<string, unknown>[];
  total: number;
  loading: boolean;
  onReload: () => void;
  onError: (message: string) => void;
  queryActive?: boolean;
  onClearQuery?: () => void;
};

export type CollectionView = (props: CollectionViewProps) => ReactNode;

const registry: Record<string, CollectionView> = {};

export function registerView(name: string, view: CollectionView) {
  registry[name] = view;
}

export function renderView(name: string, props: CollectionViewProps) {
  const View = registry[name] || registry.list;
  return createElement(View, props);
}

export function registeredViews(): ViewKind[] {
  return Object.keys(registry) as ViewKind[];
}
