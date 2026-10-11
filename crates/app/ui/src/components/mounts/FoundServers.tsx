// "Servers on your network": the file servers that announce themselves
// (mDNS / Bonjour), to pick one instead of typing its name.

import { createResource, For, Show } from 'solid-js';

import { findServers, messageOf } from '../../api';
import type { Protocol } from '../../bindings/Protocol';

export function FoundServers(props: { protocol: Protocol; onPick: (host: string) => void }) {
  const [servers, { refetch }] = createResource(findServers);

  const offering = () => (servers() ?? []).filter((server) => server.protocols.includes(props.protocol));

  const empty = () =>
    props.protocol === 'nfs'
      ? 'No servers announce NFS here (few do); type the address below.'
      : "None found. Servers that don't announce themselves (Windows PCs, for one) need their address typed below.";

  return (
    <div>
      <div class="mb-1 flex items-baseline gap-3">
        <span class="font-medium">Servers on your network</span>
        <button
          type="button"
          class="text-xs text-fg-muted underline-offset-2 hover:underline disabled:opacity-50"
          disabled={servers.loading}
          onClick={() => refetch()}
        >
          {servers.loading ? 'Looking…' : 'Look again'}
        </button>
      </div>
      <Show when={!servers.loading} fallback={<p class="text-xs text-fg-muted">Looking for servers…</p>}>
        <Show when={!servers.error} fallback={<p class="text-xs text-danger">{messageOf(servers.error)}</p>}>
          <Show when={offering().length > 0} fallback={<p class="text-xs text-fg-muted">{empty()}</p>}>
            <div class="flex flex-wrap gap-2">
              <For each={offering()}>
                {(server) => (
                  <button
                    type="button"
                    class="rounded-md border border-border px-2.5 py-1 text-left hover:bg-hover"
                    onClick={() => props.onPick(server.host)}
                  >
                    <span class="font-medium">{server.name}</span>
                    <span class="ml-2 font-mono text-xs text-fg-muted">{server.host}</span>
                  </button>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Show>
    </div>
  );
}
