import { getCurrentWindow } from '@tauri-apps/api/window';
import type { JSX } from 'solid-js';
import { createResource, createSignal, Match, onCleanup, Show, Switch } from 'solid-js';

import { helperStatus, loadState, messageOf, systemInfo, type UpdateStatus, updateStatus } from './api';
import { FolderOpen, HardDrive, Network } from './icons';
import { AboutCard } from './components/AboutCard';
import { HelperBanner } from './components/HelperBanner';
import { HelperCard } from './components/HelperCard';
import { LeaveDialog } from './components/LeaveDialog';
import { MountsPage } from './components/mounts/MountsPage';
import { NfsPage } from './components/NfsPage';
import { SambaPage } from './components/SambaPage';
import { Notice } from './components/ui';
import { confirmLeaving } from './components/unsaved';
import { UpdateBanner } from './components/UpdateBanner';

type Page = 'mounts' | 'samba' | 'nfs';

const DAY = 24 * 60 * 60 * 1000;

export function App() {
  const [info] = createResource(systemInfo);
  const [state, { refetch }] = createResource(loadState);
  const [helper, { refetch: refetchHelper }] = createResource(helperStatus);
  // At launch, then once a day. A check that fails keeps what the last one found.
  const [update, { refetch: refetchUpdate }] = createResource<UpdateStatus>(async (_, { value }) => {
    const status = await updateStatus();
    return status.kind === 'unreachable' && value?.kind === 'available' ? value : status;
  });
  const daily = setInterval(refetchUpdate, DAY);
  onCleanup(() => clearInterval(daily));
  const [page, setPage] = createSignal<Page>('mounts');

  // A form with something unsaved is lost with its page, or the window: ask first.
  const goTo = async (next: Page) => {
    if (page() !== next && (await confirmLeaving('Leave'))) setPage(next);
  };
  const closing = getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await confirmLeaving('Close'))) event.preventDefault();
  });
  onCleanup(() => void closing.then((unlisten) => unlisten()));

  const NavItem = (props: { page: Page; icon: JSX.Element; label: string; detail: string }) => (
    <button
      type="button"
      onClick={() => void goTo(props.page)}
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

  const NavHeading = (props: { children: string }) => (
    <h2 class="px-2.5 pt-3 pb-1 text-xs font-medium text-fg-muted">{props.children}</h2>
  );

  return (
    <div class="flex h-full">
      <LeaveDialog />
      <nav class="flex w-56 shrink-0 flex-col gap-1 border-r border-border bg-panel p-3">
        <h1 class="px-2.5 pt-1 pb-1 text-lg font-semibold">Lanwell</h1>
        <NavHeading>On other computers</NavHeading>
        <NavItem page="mounts" icon={<HardDrive size={18} />} label="Network folders" detail="Shares mounted here" />
        <NavHeading>Shared from here</NavHeading>
        <NavItem
          page="samba"
          icon={<FolderOpen size={18} />}
          label="Samba shares"
          detail="For Windows, Macs and phones"
        />
        <NavItem page="nfs" icon={<Network size={18} />} label="NFS exports" detail="For Linux and other Unix" />
        <div class="mt-auto">
          <HelperCard status={helper()} onChanged={refetchHelper} />
          <AboutCard status={update()} onChanged={refetchUpdate} />
        </div>
      </nav>

      <main class="flex-1 overflow-y-auto p-6">
        <UpdateBanner status={update()} />
        <HelperBanner status={helper()} onChanged={refetchHelper} />
        <Show when={state.error}>
          <Notice tone="danger">{messageOf(state.error)}</Notice>
        </Show>
        <Show when={info() && state()}>
          <Switch>
            <Match when={page() === 'mounts'}>
              <MountsPage info={info()!} mounts={state()!.mounts} onChanged={refetch} />
            </Match>
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
