import type { UiEntity } from "./types";

export function isNavCandidate(entity: UiEntity): boolean {
  return entity.standalone !== false && !entity.child_of;
}

export function primaryNavEntities(
  entities: UiEntity[],
  navigation: string[] = [],
  hidden: string[] = [],
): UiEntity[] {
  const hiddenSet = new Set(hidden);
  const candidates = entities.filter(isNavCandidate);
  if (navigation.length === 0) {
    return candidates.filter((entity) => !hiddenSet.has(entity.slug) && !hiddenSet.has(entity.entity));
  }
  const bySlug = new Map(candidates.map((entity) => [entity.slug, entity]));
  const picked = navigation.map((slug) => bySlug.get(slug)).filter(Boolean) as UiEntity[];
  const rest = candidates.filter(
    (entity) =>
      !navigation.includes(entity.slug) &&
      !hiddenSet.has(entity.slug) &&
      !hiddenSet.has(entity.entity),
  );
  return [...picked, ...rest];
}

/** Singletons and entities hidden from the primary nav — shown on Settings. */
export function settingsEntities(
  entities: UiEntity[],
  navigation: string[] = [],
  hidden: string[] = [],
): UiEntity[] {
  const hiddenSet = new Set(hidden);
  const navSet = new Set(navigation);
  return entities
    .filter(isNavCandidate)
    .filter((entity) => {
      if (entity.singleton) return true;
      if (hiddenSet.has(entity.slug) || hiddenSet.has(entity.entity)) return true;
      return false;
    })
    .filter((entity) => !navSet.has(entity.slug))
    .sort((a, b) => Number(Boolean(b.singleton)) - Number(Boolean(a.singleton)) || a.label.localeCompare(b.label));
}
