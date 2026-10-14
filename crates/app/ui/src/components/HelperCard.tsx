// The foot of the sidebar, once the helper is installed: says so, and offers
// to remove it (after asking, since lanwell can't change anything without it).

import { createSignal, Show } from 'solid-js';

import { type HelperStatus, uninstallHelper } from '../api';
import { ShieldCheck } from '../icons';
import { createAction, ErrorText } from './ui';

export function HelperCard(props: { status: HelperStatus | undefined; onChanged: () => void }) {
  const [confirming, setConfirming] = createSignal(false);
  const action = createAction();

  const version = () => (props.status?.kind === 'ready' ? props.status.version : null);

  const remove = async () => {
    if (await action.run(uninstallHelper)) {
      setConfirming(false);
      props.onChanged();
    }
  };

  const link = 'font-medium underline-offset-2 hover:underline disabled:opacity-50';

  return (
    <Show when={version()}>
      <div class="rounded-md border border-border px-2.5 py-2 text-xs">
        <div class="flex items-center gap-1.5 font-medium">
          <span class="text-accent">
            <ShieldCheck size={14} />
          </span>
          System helper installed
        </div>
        <div class="mt-0.5 text-fg-muted">Version {version()}</div>

        <Show
          when={confirming()}
          fallback={
            <button type="button" class={`mt-1.5 text-fg-muted ${link}`} onClick={() => setConfirming(true)}>
              Remove…
            </button>
          }
        >
          <p class="mt-1.5 text-fg-muted">
            Shares stay as they are, but Lanwell can't change them until the helper is installed again.
          </p>
          <div class="mt-1.5 flex gap-3">
            <button type="button" class={`text-danger ${link}`} disabled={action.busy()} onClick={remove}>
              Remove helper
            </button>
            <button type="button" class={link} disabled={action.busy()} onClick={() => setConfirming(false)}>
              Cancel
            </button>
          </div>
        </Show>
        <ErrorText message={action.error()} />
      </div>
    </Show>
  );
}
