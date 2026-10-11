import type { JSX } from 'solid-js';
import { createResource, createSignal, Match, Show, Switch } from 'solid-js';

import { loadState, messageOf, systemInfo } from './api';
import { FolderOpen, Network } from './icons';
import { NfsPage } from './components/NfsPage';
import { SambaPage } from './components/SambaPage';
import { Notice } from './components/ui';

type Page = 'samba' | 'nfs';

export function App() {
  const [info] = createResource(systemInfo);
  const [state, { refetch }] = createResource(loadState);
  const [page, setPage] = createSignal<Page>('samba');

  const NavItem = (props: { page: Page; icon: JSX.Element; label: string; detail: string }) => (
    <button
      type="button"
      onClick={() => setPage(props.page)}
      class="flex w-full items-start gap-2.5 rounded-md px-2.5 py-2 text-left hover:bg-hover"
      classList={{ 'bg-hover': page() === props.page }}
    >
      <span class="mt-0.5 text-accent">{props.icon}</span>
      <span>
        <span class="block font-medium">{props.label}</span>
        <span class="block text-xs text-fg-muted">{props.detail}</span>
      </span>
    </button>
  );

  return (
    <div class="flex h-full">
      <nav class="w-56 shrink-0 space-y-1 border-r border-border bg-panel p-3">
        <h1 class="px-2.5 pt-1 pb-3 text-lg font-semibold">netfoo</h1>
        <NavItem
          page="samba"
          icon={<FolderOpen size={18} />}
          label="Samba shares"
          detail="For Windows, Macs and phones"
        />
        <NavItem page="nfs" icon={<Network size={18} />} label="NFS exports" detail="For Linux and other Unix" />
      </nav>

      <main class="flex-1 overflow-y-auto p-6">
        <Show when={state.error}>
          <Notice tone="danger">{messageOf(state.error)}</Notice>
        </Show>
        <Show when={info() && state()}>
          <Switch>
            <Match when={page() === 'samba'}>
              <SambaPage info={info()!} shares={state()!.smbShares} onChanged={refetch} />
            </Match>
            <Match when={page() === 'nfs'}>
              <NfsPage info={info()!} exports={state()!.nfsExports} onChanged={refetch} />
            </Match>
          </Switch>
        </Show>
      </main>
    </div>
  );
}
