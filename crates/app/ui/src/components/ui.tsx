// Small building blocks shared by every page.

import type { JSX, ParentProps } from 'solid-js';
import { createMemo, createSignal, For, Show, splitProps } from 'solid-js';

import { messageOf } from '../api';
import { Pencil, X } from '../icons';
import { unsaved } from './unsaved';

export function Section(props: ParentProps<{ title: string; description?: string }>) {
  return (
    <section class="mb-8">
      <h2 class="text-base font-semibold">{props.title}</h2>
      <Show when={props.description}>
        <p class="mt-1 max-w-prose text-fg-muted">{props.description}</p>
      </Show>
      <div class="mt-3">{props.children}</div>
    </section>
  );
}

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'primary' | 'plain' | 'danger' };

export function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ['variant', 'class']);
  const look = () => {
    switch (own.variant ?? 'plain') {
      case 'primary':
        return 'bg-accent text-on-accent border-accent hover:opacity-90';
      case 'danger':
        return 'border-border text-danger hover:bg-hover';
      case 'plain':
        return 'border-border hover:bg-hover';
    }
  };
  return (
    <button
      type="button"
      {...rest}
      class={`rounded-md border px-3 py-1.5 font-medium disabled:opacity-50 ${look()} ${own.class ?? ''}`}
    />
  );
}

export const inputClass =
  'w-full rounded-md border border-border bg-field px-2.5 py-1.5 outline-none focus:border-accent';

/**
 * The section holding an add-or-edit form. Adding, it's a plain section
 * titled `title`. Editing (`editing` names what's edited), it becomes a
 * panel that can't be mistaken for adding: an accent border and tint, an
 * "Editing" label, a × to stop, and a brief ring as it appears. Either way,
 * once the form holds something that isn't saved, it says so under the
 * title (see `trackUnsaved`), where it's seen while the button isn't.
 */
export function FormSection(props: ParentProps<{ title: string; editing: string | null; onCancel: () => void }>) {
  return (
    <Show
      when={props.editing}
      keyed
      fallback={
        <Section title={props.title}>
          <UnsavedLine />
          {props.children}
        </Section>
      }
    >
      {(name) => (
        <section
          aria-label={`Editing ${name}`}
          class="mb-8 rounded-lg border border-accent bg-accent/5 px-4 pt-3 pb-4 motion-safe:animate-edit-in"
        >
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 text-xs font-semibold tracking-wide text-accent uppercase">
                <Pencil size={13} />
                Editing
              </div>
              <h2 class="mt-0.5 truncate text-base font-semibold">{name}</h2>
              <UnsavedLine fallback="Nothing changes until you save." />
            </div>
            <button
              type="button"
              class="rounded-md p-1 text-fg-muted hover:bg-hover hover:text-fg"
              aria-label="Stop editing"
              title="Stop editing"
              onClick={() => props.onCancel()}
            >
              <X size={16} />
            </button>
          </div>
          <div class="mt-3">{props.children}</div>
        </section>
      )}
    </Show>
  );
}

/** What the form holds that isn't saved yet, and that nothing changes until it is. */
function UnsavedLine(props: { fallback?: string }) {
  return (
    <Show
      when={unsaved()}
      fallback={
        <Show when={props.fallback}>
          <p class="mt-0.5 text-xs text-fg-muted">{props.fallback}</p>
        </Show>
      }
    >
      {(pending) => (
        <p class="mt-0.5 mb-3 text-xs text-warning" role="status">
          {pending().what} Nothing changes until you press {pending().button}.
        </p>
      )}
    </Show>
  );
}

/** A labelled form control. */
export function Field(props: ParentProps<{ label: string; hint?: string }>) {
  return (
    <label class="block">
      <span class="mb-1 block font-medium">{props.label}</span>
      {props.children}
      <Show when={props.hint}>
        <span class="mt-1 block text-xs text-fg-muted">{props.hint}</span>
      </Show>
    </label>
  );
}

/** A choice between a few options, side by side, like SMB / NFS. */
export function Segmented<T extends string>(props: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  /** What's being chosen, for screen readers. */
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={props.label} class="inline-flex rounded-md border border-border p-0.5">
      <For each={props.options}>
        {(option) => (
          <button
            type="button"
            role="radio"
            aria-checked={props.value === option.value}
            onClick={() => props.onChange(option.value)}
            class="rounded px-3 py-1 font-medium"
            classList={{
              'bg-accent text-on-accent': props.value === option.value,
              'text-fg-muted hover:bg-hover': props.value !== option.value,
            }}
          >
            {option.label}
          </button>
        )}
      </For>
    </div>
  );
}

/** A short label beside a name, like a mount's protocol (SMB). */
export function Tag(props: { children: string }) {
  return (
    <span class="rounded border border-border px-1.5 py-px align-middle text-[0.6875rem] font-medium tracking-wide text-fg-muted">
      {props.children}
    </span>
  );
}

/** Something the person should know before they go on. */
export function Notice(props: ParentProps<{ tone?: 'warning' | 'danger' }>) {
  const tone = () => (props.tone === 'danger' ? 'border-danger text-danger' : 'border-warning text-warning');
  return <div class={`mb-4 rounded-md border px-3 py-2 ${tone()}`}>{props.children}</div>;
}

/** A list of things, or what to say when there are none. */
export function ItemList<T>(props: {
  items: T[];
  empty: string;
  /** Marks an item, like the one being edited, with a tint and an accent bar. */
  highlighted?: (item: T) => boolean;
  children: (item: T) => JSX.Element;
}) {
  return (
    <Show when={props.items.length > 0} fallback={<p class="text-fg-muted italic">{props.empty}</p>}>
      <ul class="divide-y divide-border rounded-md border border-border">
        <For each={props.items}>
          {(item) => (
            <li
              class="flex items-center gap-3 px-3 py-2"
              classList={{ 'bg-accent/10 shadow-[inset_3px_0_0_var(--accent)]': props.highlighted?.(item) ?? false }}
            >
              {props.children(item)}
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
}

/**
 * Runs changes one at a time, keeping what the UI shows while they run: busy
 * (disable the controls) and the last one's error message, if it failed.
 * Resolves to whether it succeeded.
 */
export function createAction() {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const run = async (change: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await change();
      return true;
    } catch (e) {
      setError(messageOf(e));
      return false;
    } finally {
      setBusy(false);
    }
  };

  /** Forgets the last error (when what it was about is gone). */
  const reset = () => setError(null);

  return { busy, error, run, reset };
}

/** The command that installs or updates Lanwell and its helper; a click selects all of it, to copy. */
export function InstallCommand() {
  return (
    <code class="mt-2 block rounded bg-field px-2 py-1 text-xs break-all select-all">
      curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh
    </code>
  );
}

export function ErrorText(props: { message: string | null }) {
  return (
    <Show when={props.message}>
      <p class="mt-2 text-danger">{props.message}</p>
    </Show>
  );
}

/**
 * An "add or edit" form's bookkeeping: what's being edited (null when adding a
 * new one), and `form()`, which changes whenever the form should start afresh
 * (editing starts or ends, or a save goes through), for a keyed `<Show>` to
 * rebuild it from.
 */
export function createEditing<T>() {
  const [editing, setEditing] = createSignal<T | null>(null);
  const [round, setRound] = createSignal(0);
  const form = createMemo(() => ({ editing: editing(), round: round() }));

  return {
    editing,
    form,
    edit: (item: T) => setEditing(() => item),
    /** Saved or cancelled: back to an empty form for adding. */
    done: () => {
      setEditing(null);
      setRound((round) => round + 1);
    },
  };
}
