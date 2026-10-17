// What a form holds that isn't saved yet, so the form can say so and leaving
// the page or closing the window can ask first. One form at a time has
// anything unsaved: each page has one.

import { ask } from '@tauri-apps/plugin-dialog';
import { createEffect, createSignal, onCleanup } from 'solid-js';

export interface Unsaved {
  /** What isn't saved, as a sentence: “Media” isn't shared yet. */
  what: string;
  /** The button that saves it: Share, Export, Connect or Save. */
  button: string;
}

const [unsaved, setUnsaved] = createSignal<Unsaved | null>(null);

/** What the form on screen holds that isn't saved, or null. */
export { unsaved };

/**
 * Keeps `unsaved` up to date from a form. `describe` returns what the form
 * holds that isn't saved, or null when it's empty or the same as what's
 * saved. Call it in the form's body; it's cleared when the form goes away.
 */
export function trackUnsaved(describe: () => Unsaved | null) {
  createEffect(() => setUnsaved(describe()));
  onCleanup(() => setUnsaved(null));
}

/**
 * Whether to go ahead and `verb` ("Leave", "Close"): yes when nothing is
 * unsaved, otherwise whatever the person answers.
 */
export async function confirmLeaving(verb: string): Promise<boolean> {
  const pending = unsaved();
  if (!pending) return true;
  return ask(`${pending.what} ${verb} anyway?`, {
    title: 'Lanwell',
    kind: 'warning',
    okLabel: verb,
    cancelLabel: 'Stay',
  });
}

/** Whether two saved things are the same, whatever order their fields came in. */
export function same(a: unknown, b: unknown): boolean {
  return canonical(a) === canonical(b);
}

function canonical(value: unknown): string {
  return JSON.stringify(value, (_, field: unknown) =>
    field && typeof field === 'object' && !Array.isArray(field)
      ? Object.fromEntries(Object.entries(field).sort(([a], [b]) => a.localeCompare(b)))
      : field,
  );
}
