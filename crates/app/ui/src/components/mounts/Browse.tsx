// "Show shares on nas.lan": asks a server what it offers, to pick one.

import { createSignal, For, Show } from 'solid-js';

import { Button, createAction, ErrorText } from '../ui';

export function Browse(props: {
  host: string;
  /** What the server offers ("shares", "folders"), for the button and the empty message. */
  noun: string;
  list: () => Promise<string[]>;
  onPick: (item: string) => void;
}) {
  const [found, setFound] = createSignal<string[] | null>(null);
  const action = createAction();

  const browse = () =>
    action.run(async () => {
      setFound(null);
      setFound(await props.list());
    });

  return (
    <div>
      <Button disabled={action.busy() || props.host === ''} onClick={browse}>
        {action.busy() ? 'Looking…' : `Show ${props.noun} on ${props.host || 'the server'}`}
      </Button>
      <ErrorText message={action.error()} />
      <Show when={found()}>
        {(items) => (
          <Show
            when={items().length > 0}
            fallback={<p class="mt-2 text-fg-muted">The server has no {props.noun} to show.</p>}
          >
            <div class="mt-2 flex flex-wrap gap-2">
              <For each={items()}>
                {(item) => (
                  <button
                    type="button"
                    class="rounded-md border border-border px-2.5 py-1 font-mono text-xs hover:bg-hover"
                    onClick={() => props.onPick(item)}
                  >
                    {item}
                  </button>
                )}
              </For>
            </div>
          </Show>
        )}
      </Show>
    </div>
  );
}
