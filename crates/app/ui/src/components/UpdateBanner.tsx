// Says when a newer lanwell is out, and how to install it: run install.sh
// again. "Later" hides it until lanwell next starts (or a newer one is out).

import { createSignal, Show } from 'solid-js';

import { openReleaseNotes, type UpdateStatus } from '../api';
import { Button, createAction, ErrorText, InstallCommand } from './ui';

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
              <p>Lanwell {version()} is out. To update, run the installer again, then reopen Lanwell:</p>
              <InstallCommand />
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
