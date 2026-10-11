// Small building blocks shared by every page.

import type { JSX, ParentProps } from 'solid-js';
import { createSignal, For, Show, splitProps } from 'solid-js';

import { messageOf } from '../api';

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
export function ItemList<T>(props: { items: T[]; empty: string; children: (item: T) => JSX.Element }) {
  return (
    <Show when={props.items.length > 0} fallback={<p class="text-fg-muted italic">{props.empty}</p>}>
      <ul class="divide-y divide-border rounded-md border border-border">
        <For each={props.items}>
          {(item) => <li class="flex items-center gap-3 px-3 py-2">{props.children(item)}</li>}
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

export function ErrorText(props: { message: string | null }) {
  return (
    <Show when={props.message}>
      <p class="mt-2 text-danger">{props.message}</p>
    </Show>
  );
}
