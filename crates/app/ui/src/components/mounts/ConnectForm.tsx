// "Connect to a share": an SMB share or an NFS export, mounted as a folder here.

import { createSignal, For, Match, Show, Switch } from 'solid-js';

import { chooseFolder, listServerExports, listServerShares, type Mount, setMount, type SystemInfo } from '../../api';
import type { NfsVersion } from '../../bindings/NfsVersion';
import type { SmbVersion } from '../../bindings/SmbVersion';
import { Button, createAction, ErrorText, Field, inputClass, Notice, Segmented } from '../ui';
import { lastName, parseNfsAddress, parseSmbAddress, tildify } from './addresses';
import { Browse } from './Browse';

type Protocol = 'smb' | 'nfs';

const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: 'smb', label: 'SMB' },
  { value: 'nfs', label: 'NFS' },
];

const SMB_VERSIONS: { value: SmbVersion; label: string }[] = [
  { value: 'automatic', label: 'Automatic (recommended)' },
  { value: '3.1.1', label: 'SMB 3.1.1' },
  { value: '3.0', label: 'SMB 3.0' },
  { value: '2.1', label: 'SMB 2.1' },
  { value: '2.0', label: 'SMB 2.0' },
  { value: '1.0', label: 'SMB 1 (old and insecure; very old devices only)' },
];

const NFS_VERSIONS: { value: NfsVersion; label: string }[] = [
  { value: 'automatic', label: 'Automatic (recommended)' },
  { value: '4.2', label: 'NFS 4.2' },
  { value: '4.1', label: 'NFS 4.1' },
  { value: '4.0', label: 'NFS 4.0' },
  { value: '3', label: 'NFS 3 (macOS, older NAS boxes)' },
];

export function ConnectForm(props: { info: SystemInfo; onConnected: () => void }) {
  const [protocol, setProtocol] = createSignal<Protocol>('smb');
  const [address, setAddress] = createSignal('');
  // SMB only: how to sign in.
  const [asGuest, setAsGuest] = createSignal(false);
  const [username, setUsername] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [domain, setDomain] = createSignal('');
  // Follows the share's name until the person sets a folder themselves.
  const [customFolder, setCustomFolder] = createSignal<string | null>(null);
  const [atStartup, setAtStartup] = createSignal(false);
  const [readOnly, setReadOnly] = createSignal(false);
  const [smbVersion, setSmbVersion] = createSignal<SmbVersion>('automatic');
  const [nfsVersion, setNfsVersion] = createSignal<NfsVersion>('automatic');
  const [connected, setConnected] = createSignal<string | null>(null);
  const action = createAction();

  const smb = () => parseSmbAddress(address());
  const nfs = () => parseNfsAddress(address());
  const host = () => (protocol() === 'smb' ? smb().host : nfs().host);
  /** What the folder is named after: the last part of the address (`media`, or `films` in `media/films`). */
  const shareName = () => lastName(protocol() === 'smb' ? smb().share : nfs().path);

  const suggestedFolder = () =>
    props.info.homeDir && shareName() ? `${props.info.homeDir}/Network/${shareName()}` : '';
  const folder = () => customFolder() ?? suggestedFolder();

  const switchTo = (next: Protocol) => {
    // The two are written differently, so an address typed for one won't do for the other.
    setProtocol(next);
    setAddress('');
    setConnected(null);
  };

  const pickFolder = async () => {
    const chosen = await chooseFolder();
    if (chosen) setCustomFolder(chosen);
  };

  const mount = (): Mount => ({
    share:
      protocol() === 'smb'
        ? {
            protocol: 'smb',
            address: address().trim(),
            login: asGuest()
              ? { kind: 'guest' }
              : { kind: 'user', username: username().trim(), domain: domain().trim() || null },
            version: smbVersion(),
          }
        : { protocol: 'nfs', address: address().trim(), version: nfsVersion() },
    mountPoint: folder(),
    when: atStartup() ? 'startup' : 'firstAccess',
    readOnly: readOnly(),
  });

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    setConnected(null);
    const usesPassword = protocol() === 'smb' && !asGuest();
    const succeeded = await action.run(() => setMount(mount(), usesPassword ? password() : null));
    // A mount that's saved but didn't connect is listed too (with its error here).
    props.onConnected();
    if (succeeded) {
      setConnected(`Connected. ${shareName()} is at ${tildify(folder(), props.info.homeDir)}.`);
      setAddress('');
      setPassword('');
      setCustomFolder(null);
    }
  };

  const ready = () => {
    const signedIn = protocol() === 'nfs' || asGuest() || username().trim() !== '';
    return host() !== '' && shareName() !== '' && folder() !== '' && signedIn;
  };

  return (
    <form class="max-w-md space-y-4" onSubmit={submit}>
      <div>
        <Segmented options={PROTOCOLS} value={protocol()} onChange={switchTo} label="Protocol" />
        <p class="mt-1.5 text-xs text-fg-muted">
          {protocol() === 'smb'
            ? 'For Windows, Macs and most NAS boxes.'
            : 'For Linux and other Unix machines, and NAS boxes set up for it.'}
        </p>
      </div>

      <Switch>
        <Match when={protocol() === 'smb' && !props.info.cifsInstalled}>
          <Notice>
            Mounting SMB shares needs <code>cifs-utils</code>. Install your distribution's <code>cifs-utils</code>{' '}
            package.
          </Notice>
        </Match>
        <Match when={protocol() === 'nfs' && !props.info.nfsClientInstalled}>
          <Notice>
            Mounting NFS needs <code>nfs-utils</code> (<code>nfs-common</code> on Debian and Ubuntu).
          </Notice>
        </Match>
      </Switch>

      <Field
        label="Address"
        hint={
          protocol() === 'smb'
            ? 'The server and the share on it. Add a folder to mount just that: //nas.lan/media/films.'
            : 'The server and the folder it exports.'
        }
      >
        <input
          class={inputClass}
          placeholder={protocol() === 'smb' ? '//nas.lan/media' : 'nas.lan:/srv/media'}
          value={address()}
          onInput={(e) => setAddress(e.currentTarget.value)}
        />
      </Field>

      <Switch>
        <Match when={protocol() === 'smb'}>
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
            <Browse
              host={smb().host}
              noun="shares"
              list={() =>
                listServerShares(
                  smb().host,
                  asGuest() ? null : username().trim() || null,
                  asGuest() ? null : password(),
                )
              }
              onPick={(share) => setAddress(`//${smb().host}/${share}`)}
            />
          </Show>
        </Match>

        <Match when={protocol() === 'nfs'}>
          <div class="space-y-1.5 rounded-md border border-border px-3 py-2 text-xs text-fg-muted">
            <p>
              <span class="font-medium text-fg">No sign-in.</span> The server decides which machines may mount its
              folders, so this computer needs to be on its export list.
            </p>
            <p>
              Files keep the server's owners: they're yours only if your user ID on the server is also{' '}
              {props.info.userId ?? 'the same as here'}.
            </p>
          </div>

          <Show when={props.info.showmountInstalled}>
            <Browse
              host={nfs().host}
              noun="folders"
              list={() => listServerExports(nfs().host)}
              onPick={(path) => setAddress(`${nfs().host}:${path}`)}
            />
          </Show>
        </Match>
      </Switch>

      <details class="rounded-md border border-border px-3 py-2">
        <summary class="cursor-pointer font-medium">More options</summary>
        <div class="mt-3 space-y-4">
          <Show when={protocol() === 'smb' && !asGuest()}>
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

          <Switch>
            <Match when={protocol() === 'smb'}>
              <VersionField label="SMB version" options={SMB_VERSIONS} value={smbVersion()} onChange={setSmbVersion} />
            </Match>
            <Match when={protocol() === 'nfs'}>
              <VersionField label="NFS version" options={NFS_VERSIONS} value={nfsVersion()} onChange={setNfsVersion} />
            </Match>
          </Switch>
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

function VersionField<T extends string>(props: {
  label: string;
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <Field label={props.label} hint="Change only if a server won't connect with Automatic.">
      <select class={inputClass} value={props.value} onChange={(e) => props.onChange(e.currentTarget.value as T)}>
        <For each={props.options}>{(option) => <option value={option.value}>{option.label}</option>}</For>
      </select>
    </Field>
  );
}
