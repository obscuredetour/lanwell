// Folders shared over NFS.

import { createSignal, Show } from 'solid-js';

import { chooseFolder, type NfsExport, setNfsExports, type SystemInfo } from '../api';
import { Button, createAction, ErrorText, Field, inputClass, ItemList, Notice, Section } from './ui';

export function NfsPage(props: { info: SystemInfo; exports: NfsExport[]; onChanged: () => void }) {
  const action = createAction();

  /** Saves the whole list (the helper replaces what it had with it). */
  const save = async (exports: NfsExport[]) => {
    if (await action.run(() => setNfsExports(exports))) props.onChanged();
  };

  return (
    <>
      <Show when={!props.info.nfsServerInstalled}>
        <Notice>
          The NFS server isn't installed. Install your distribution's <code>nfs-utils</code> (or{' '}
          <code>nfs-kernel-server</code>) package to export folders over NFS.
        </Notice>
      </Show>

      <Section
        title="Exported folders"
        description="NFS has no user names or passwords: it trusts the machines you list, and their users. List only machines you trust, or use Samba."
      >
        <ItemList items={props.exports} empty="No folders are exported yet.">
          {(nfsExport) => (
            <>
              <div class="min-w-0 flex-1">
                <div class="truncate font-mono">{nfsExport.path}</div>
                <div class="text-xs text-fg-muted">
                  {nfsExport.clients.join(', ')}
                  {nfsExport.readOnly ? ' · read only' : ''}
                </div>
              </div>
              <Button
                variant="danger"
                disabled={action.busy()}
                onClick={() => save(props.exports.filter((other) => other !== nfsExport))}
              >
                Stop exporting
              </Button>
            </>
          )}
        </ItemList>
        <ErrorText message={action.error()} />
      </Section>

      <Section title="Export a folder">
        <AddExport busy={action.busy()} onAdd={(nfsExport) => save([...props.exports, nfsExport])} />
      </Section>
    </>
  );
}

function AddExport(props: { busy: boolean; onAdd: (nfsExport: NfsExport) => void }) {
  const [path, setPath] = createSignal('');
  const [clients, setClients] = createSignal('');
  const [readOnly, setReadOnly] = createSignal(false);

  const pickFolder = async () => {
    const folder = await chooseFolder();
    if (folder) setPath(folder);
  };

  /** The clients as typed: separated by spaces or commas. */
  const clientList = () =>
    clients()
      .split(/[\s,]+/)
      .filter(Boolean);

  const add = (event: SubmitEvent) => {
    event.preventDefault();
    props.onAdd({ path: path(), readOnly: readOnly(), clients: clientList() });
  };

  return (
    <form class="max-w-md space-y-4" onSubmit={add}>
      <Field label="Folder">
        <div class="flex gap-2">
          <input class={inputClass} value={path()} onInput={(e) => setPath(e.currentTarget.value)} />
          <Button onClick={pickFolder}>Choose…</Button>
        </div>
      </Field>

      <Field
        label="Machines that may mount it"
        hint="Host names, addresses or networks, like laptop.lan or 192.168.1.0/24. Use * for any machine."
      >
        <input class={inputClass} value={clients()} onInput={(e) => setClients(e.currentTarget.value)} />
      </Field>

      <label class="flex items-center gap-2">
        <input type="checkbox" checked={readOnly()} onChange={(e) => setReadOnly(e.currentTarget.checked)} />
        Read only
      </label>

      <Show when={!readOnly() && clientList().includes('*')}>
        <p class="text-xs text-warning">
          Any machine on the network will be able to change these files, as any user but root: NFS trusts what each
          machine says about who its users are. Consider listing your own machines, or making it read only.
        </p>
      </Show>

      <Button type="submit" variant="primary" disabled={props.busy || !path() || clientList().length === 0}>
        Export
      </Button>
    </form>
  );
}
