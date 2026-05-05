import type { ComponentType } from "react";

export type TabId = string;

export interface TabEntry {
  id: TabId;
  title: string;
  component: ComponentType;
  order: number;
}

const registry = new Map<TabId, TabEntry>();
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((fn) => fn());
}

export function registerTab(entry: TabEntry): void {
  registry.set(entry.id, entry);
  emit();
}

export function unregisterTab(id: TabId): void {
  registry.delete(id);
  emit();
}

export function listTabs(): TabEntry[] {
  return Array.from(registry.values()).sort((a, b) => a.order - b.order);
}

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function getSnapshot(): TabEntry[] {
  return listTabs();
}
