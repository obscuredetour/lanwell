// Shares on other computers, mounted as folders on this one.

import { createResource, createSignal, For, onCleanup, Show } from 'solid-js';

import {
  chooseFolder,
  listServerShares,
  type ManagedMount,
  type MountStatus,
  mountStatuses,
  openMount,
  removeSmbMount,
  setSmbMount,
  type SmbMount,
  type SystemInfo,
} from '../api';
import type { SmbVersion } from '../bindings/SmbVersion';
import { Button, createAction, ErrorText, Field, inputClass, ItemList, Notice, Section, Tag } from './ui';

/** How often the list checks what each mount is doing. */
const STATUS_REFRESH_MS = 5000;

export function MountsPage(props: { info: SystemInfo; mounts: ManagedMount[]; onChanged: () => void }) {
  const [statuses, { refetch: refetchStatuses }] = createResource(mountStatuses, { initialValue: [] });
  const timer = setInterval(refetchStatuses, STATUS_REFRESH_MS);
  onCleanup(() => clearInterval(timer));

  const action = createAction();

  const changed = () => {
    props.onChanged();
    refetchStatuses();
  };

  const remove = async (mountPoint: string) => {
    if (await action.run(() => removeSmbMount(mountPoint))) changed();
  };

  const statusOf = (mountPoint: string) => statuses().find((status) => status.mountPoint === mountPoint);

  return (
    <>
      <Show when={!props.info.cifsInstalled}>
        <Notice>
          Mounting shares needs <code>cifs-utils</code>. Install your distribution's <code>cifs-utils</code> package.
        </Notice>
      </Show>

      <Section title="Network folders" description="Shares on other computers, as folders on this one.">
        <ItemList items={props.mounts} empty="No shares are mounted yet.">
          {(managed) => (
            <>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="font-medium">{managed.address}</span>
                  {/* Every mount is SMB until NFS mounts arrive; then this comes from the mount. */}
                  <Tag>SMB</Tag>
                </div>
                <div class="truncate font-mono text-xs text-fg-muted">
                  {tildify(managed.mountPoint, props.info.homeDir)}
                </div>
              </div>
              <StatusLabel status={statusOf(managed.mountPoint)} />
              <Button disabled={action.busy()} onClick={() => action.run(() => openMount(managed.mountPoint))}>
                Open
              </Button>
              <Button variant="danger" disabled={action.busy()} onClick={() => remove(managed.mountPoint)}>
                Remove
              </Button>
            </>
          )}
        </ItemList>
        <ErrorText message={action.error()} />
      </Section>

      <Section title="Connect to a share">
        <ConnectForm info={props.info} onConnected={changed} />
      </Section>
    </>
  );
}

function StatusLabel(props: { status: MountStatus | undefined }) {
  const label = () => {
    switch (props.status?.activity) {
      case 'mounted':
        return { text: 'Connected', class: 'text-accent' };
      case 'waiting':
        return { text: 'Connects when opened', class: 'text-fg-muted' };
      case 'failed':
        return { text: "Couldn't connect", class: 'text-danger' };
      default:
        return { text: 'Off', class: 'text-fg-muted' };
    }
  };
  return <span class={`text-xs ${label().class}`}>{label().text}</span>;
}

/** `/home/alice/Network/media` as `~/Network/media`. */
function tildify(path: string, home: string | null) {
  return home && path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}

/** The server and share in an address as typed (`//nas.lan/media`, `\\nas.lan\media`, `smb://…`). */
function parseAddress(address: string) {
  const rest = address
    .trim()
    .replace(/^(smb:\/\/|\/\/|\\\\)/, '')
    .replaceAll('\\', '/');
  const [host = '', share = ''] = rest.split('/');
  return { host, share };
}

const VERSIONS: { value: SmbVersion; label: string }[] = [
  { value: 'automatic', label: 'Automatic (recommended)' },
  { value: '3.1.1', label: 'SMB 3.1.1' },
  { value: '3.0', label: 'SMB 3.0' },
  { value: '2.1', label: 'SMB 2.1' },
  { value: '2.0', label: 'SMB 2.0' },
  { value: '1.0', label: 'SMB 1 (old and insecure; very old devices only)' },
];

function ConnectForm(props: { info: SystemInfo; onConnected: () => void }) {
  const [address, setAddress] = createSignal('');
  const [asGuest, setAsGuest] = createSignal(false);
  const [username, setUsername] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [domain, setDomain] = createSignal('');
  // Follows the share's name until the person sets a folder themselves.
  const [customFolder, setCustomFolder] = createSignal<string | null>(null);
  const [atStartup, setAtStartup] = createSignal(false);
  const [readOnly, setReadOnly] = createSignal(false);
  const [version, setVersion] = createSignal<SmbVersion>('automatic');
  const [connected, setConnected] = createSignal<string | null>(null);
  const action = createAction();

  const parsed = () => parseAddress(address());
  const suggestedFolder = () =>
    props.info.homeDir && parsed().share ? `${props.info.homeDir}/Network/${parsed().share}` : '';
  const folder = () => customFolder() ?? suggestedFolder();

  const pickFolder = async () => {
    const chosen = await chooseFolder();
    if (chosen) setCustomFolder(chosen);
  };

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    setConnected(null);
    const mount: SmbMount = {
      address: address().trim(),
      mountPoint: folder(),
      login: asGuest()
        ? { kind: 'guest' }
        : { kind: 'user', username: username().trim(), domain: domain().trim() || null },
      when: atStartup() ? 'startup' : 'firstAccess',
      readOnly: readOnly(),
      version: version(),
    };
    const succeeded = await action.run(() => setSmbMount(mount, asGuest() ? null : password()));
    // A mount that's saved but didn't connect is listed too (with its error here).
    props.onConnected();
    if (succeeded) {
      setConnected(`Connected. ${parsed().share} is at ${tildify(folder(), props.info.homeDir)}.`);
      setAddress('');
      setPassword('');
      setCustomFolder(null);
    }
  };

  const ready = () =>
    parsed().host !== '' && parsed().share !== '' && folder() !== '' && (asGuest() || username().trim() !== '');

  return (
    <form class="max-w-md space-y-4" onSubmit={submit}>
      <Field label="Address" hint="The server and the share on it.">
        <input
          class={inputClass}
          placeholder="//nas.lan/media"
          value={address()}
          onInput={(e) => setAddress(e.currentTarget.value)}
        />
      </Field>

      <fieldset class="space-y-1.5">
        <legend class="mb-1 font-medium">Sign in</legend>
        <label class="flex items-center gap-2">
          <input type="radio" checked={!asGuest()} onChange={() => setAsGuest(false)} />
          With a user name and password
        </label>
        <label class="flex items-center gap-2">
          <input type="radio" checked={asGuest()} onChange={() => setAsGuest(true)} />
          As a guest
        </label>
      </fieldset>

      <Show when={!asGuest()}>
        <Field label="User name">
          <input class={inputClass} value={username()} onInput={(e) => setUsername(e.currentTarget.value)} />
        </Field>
        <Field label="Password" hint="Kept in a file only the system can read.">
          <input
            type="password"
            class={inputClass}
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
          />
        </Field>
      </Show>

      <Show when={props.info.smbclientInstalled}>
        <BrowseShares
          host={parsed().host}
          username={asGuest() ? null : username().trim() || null}
          password={asGuest() ? null : password()}
          onPick={(share) => setAddress(`//${parsed().host}/${share}`)}
        />
      </Show>

      <details class="rounded-md border border-border px-3 py-2">
        <summary class="cursor-pointer font-medium">More options</summary>
        <div class="mt-3 space-y-4">
          <Show when={!asGuest()}>
            <Field label="Domain or workgroup" hint="Only if the server asks for one.">
              <input class={inputClass} value={domain()} onInput={(e) => setDomain(e.currentTarget.value)} />
            </Field>
          </Show>

          <Field label="Folder" hint="Where the share appears on this computer.">
            <div class="flex gap-2">
              <input
                class={inputClass}
                placeholder="~/Network/<share>"
                value={folder()}
                onInput={(e) => setCustomFolder(e.currentTarget.value)}
              />
              <Button onClick={pickFolder}>Choose…</Button>
            </div>
          </Field>

          <fieldset class="space-y-1.5">
            <legend class="mb-1 font-medium">Connect</legend>
            <label class="flex items-center gap-2">
              <input type="radio" checked={!atStartup()} onChange={() => setAtStartup(false)} />
              When the folder is first opened (recommended)
            </label>
            <label class="flex items-center gap-2">
              <input type="radio" checked={atStartup()} onChange={() => setAtStartup(true)} />
              While the computer starts up
            </label>
            <Show when={atStartup()}>
              <p class="pl-6 text-xs text-warning">
                Starting up will wait for this server. If it's off or unreachable, startup takes up to 30 seconds
                longer, every time. Choose this only if something needs the files before you open them.
              </p>
            </Show>
          </fieldset>

          <label class="flex items-center gap-2">
            <input type="checkbox" checked={readOnly()} onChange={(e) => setReadOnly(e.currentTarget.checked)} />
            Read only
          </label>

          <Field label="SMB version" hint="Change only if a server won't connect with Automatic.">
            <select
              class={inputClass}
              value={version()}
              onChange={(e) => setVersion(e.currentTarget.value as SmbVersion)}
            >
              <For each={VERSIONS}>{(option) => <option value={option.value}>{option.label}</option>}</For>
            </select>
          </Field>
        </div>
      </details>

      <Button type="submit" variant="primary" disabled={action.busy() || !ready()}>
        {action.busy() ? 'Connecting…' : 'Connect'}
      </Button>
      <ErrorText message={action.error()} />
      <Show when={connected()}>
        <p class="text-accent">{connected()}</p>
      </Show>
    </form>
  );
}

/** Lists the shares on the server in the address, to pick one. */
function BrowseShares(props: {
  host: string;
  username: string | null;
  password: string | null;
  onPick: (share: string) => void;
}) {
  const [shares, setShares] = createSignal<string[] | null>(null);
  const action = createAction();

  const browse = () =>
    action.run(async () => {
      setShares(null);
      setShares(await listServerShares(props.host, props.username, props.password));
    });

  return (
    <div>
      <Button disabled={action.busy() || props.host === ''} onClick={browse}>
        {action.busy() ? 'Looking…' : `Show shares on ${props.host || 'the server'}`}
      </Button>
      <ErrorText message={action.error()} />
      <Show when={shares()}>
        {(found) => (
          <Show
            when={found().length > 0}
            fallback={<p class="mt-2 text-fg-muted">The server has no shared folders.</p>}
          >
            <div class="mt-2 flex flex-wrap gap-2">
              <For each={found()}>
                {(share) => (
                  <button
                    type="button"
                    class="rounded-md border border-border px-2.5 py-1 font-mono text-xs hover:bg-hover"
                    onClick={() => props.onPick(share)}
                  >
                    {share}
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
