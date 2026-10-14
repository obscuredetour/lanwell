// Says when a newer lanwell is out, and how to install it: run install.sh
// again. "Later" hides it until lanwell next starts (or a newer one is out).

import { createSignal, Show } from 'solid-js';

import { openReleaseNotes, type UpdateStatus } from '../api';
import { Button, createAction, ErrorText } from './ui';

const INSTALL_COMMAND = 'curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh';

export function UpdateBanner(props: { status: UpdateStatus | undefined }) {
  const [dismissed, setDismissed] = createSignal<string | null>(null);
  const action = createAction();

  const version = () =>
    props.status?.kind === 'available' && props.status.version !== dismissed() ? props.status.version : null;

  return (
    <Show when={version()}>
      {(version) => (
        <div class="mb-6 rounded-md border border-accent px-4 py-3">
          <div class="flex items-start gap-4">
            <div class="min-w-0 flex-1">
              <p>lanwell {version()} is out. To update, run the installer again, then reopen lanwell:</p>
              <code class="mt-2 block rounded bg-field px-2 py-1 text-xs break-all select-all">{INSTALL_COMMAND}</code>
            </div>
            <div class="flex shrink-0 gap-2">
              <Button disabled={action.busy()} onClick={() => action.run(openReleaseNotes)}>
                What's new
              </Button>
              <Button onClick={() => setDismissed(version())}>Later</Button>
            </div>
          </div>
          <ErrorText message={action.error()} />
        </div>
      )}
    </Show>
  );
}
