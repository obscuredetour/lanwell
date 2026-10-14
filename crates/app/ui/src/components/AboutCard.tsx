// The very foot of the sidebar: lanwell's version and, for a copy install.sh
// installed, whether it checks for new ones.

import { getVersion } from '@tauri-apps/api/app';
import { createResource, Show } from 'solid-js';

import { setUpdateChecks, type UpdateStatus } from '../api';
import { createAction, ErrorText } from './ui';

export function AboutCard(props: { status: UpdateStatus | undefined; onChanged: () => void }) {
  const [version] = createResource(getVersion);
  const action = createAction();

  const unreachable = () => (props.status?.kind === 'unreachable' ? props.status.message : null);

  const toggle = async (on: boolean) => {
    if (await action.run(() => setUpdateChecks(on))) props.onChanged();
  };

  return (
    <div class="px-2.5 pt-2 text-xs text-fg-muted">
      <Show when={version()}>
        <div>Lanwell {version()}</div>
      </Show>
      <Show when={props.status && props.status.kind !== 'unmanaged'}>
        <label class="mt-1 flex items-center gap-2">
          <input
            type="checkbox"
            checked={props.status?.kind !== 'off'}
            disabled={action.busy()}
            onChange={(e) => toggle(e.currentTarget.checked)}
          />
          Check for new versions
        </label>
        <Show when={unreachable()}>{(message) => <div title={message()}>Couldn't check just now.</div>}</Show>
      </Show>
      <ErrorText message={action.error()} />
    </div>
  );
}
