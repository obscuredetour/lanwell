// Folders shared over NFS.

import { createSignal, Show } from 'solid-js';

import { chooseFolder, type NfsExport, setNfsExports, type SystemInfo } from '../api';
import { ServerStatus } from './ServerStatus';
import {
  Button,
  createAction,
  createEditing,
  ErrorText,
  Field,
  FormSection,
  inputClass,
  ItemList,
  Notice,
  Section,
} from './ui';

export function NfsPage(props: { info: SystemInfo; exports: NfsExport[]; onChanged: () => void }) {
  // Separate, so an error shows next to what caused it.
  const listAction = createAction();
  const formAction = createAction();
  const exportForm = createEditing<NfsExport>();
  let formTop: HTMLDivElement | undefined;

  /** Saves the whole list (the helper replaces what it had with it). */
  const save = async (action: ReturnType<typeof createAction>, exports: NfsExport[]) => {
    const saved = await action.run(() => setNfsExports(exports));
    if (saved) props.onChanged();
    return saved;
  };

  /** A new export, or the one being edited (found by its folder, which is unique) changed. */
  const saveExport = async (nfsExport: NfsExport) => {
    const original = exportForm.editing();
    const exports = original
      ? props.exports.map((other) => (other.path === original.path ? nfsExport : other))
      : [...props.exports, nfsExport];
    if (await save(formAction, exports)) exportForm.done();
  };

  const stopEditing = () => {
    formAction.reset();
    exportForm.done();
  };

  /** Whether `nfsExport` is the one the form below is editing. */
  const isEditing = (nfsExport: NfsExport) => exportForm.editing()?.path === nfsExport.path;

  const startEditing = (nfsExport: NfsExport) => {
    formAction.reset();
    exportForm.edit(nfsExport);
    formTop?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  return (
    <>
      <Show when={!props.info.nfsServerInstalled}>
        <Notice>
          The NFS server isn't installed. Install your distribution's <code>nfs-utils</code> (or{' '}
          <code>nfs-kernel-server</code>) package to export folders over NFS.
        </Notice>
      </Show>

      <ServerStatus server="nfs" shared={props.exports.length} />

      <Section
        title="Exported folders"
        description="NFS has no user names or passwords: it trusts the machines you list, and their users. List only machines you trust, or use Samba."
      >
        <ItemList items={props.exports} empty="No folders are exported yet." highlighted={isEditing}>
          {(nfsExport) => (
            <>
              <div class="min-w-0 flex-1">
                <div class="truncate font-mono">{nfsExport.path}</div>
                <div class="text-xs text-fg-muted">
                  {nfsExport.clients.join(', ')}
                  {nfsExport.readOnly ? ' · read only' : ''}
                </div>
              </div>
              <Button disabled={listAction.busy() || isEditing(nfsExport)} onClick={() => startEditing(nfsExport)}>
                {isEditing(nfsExport) ? 'Editing' : 'Edit'}
              </Button>
              <Button
                variant="danger"
                disabled={listAction.busy()}
                onClick={() =>
                  save(
                    listAction,
                    props.exports.filter((other) => other !== nfsExport),
                  )
                }
              >
                Stop exporting
              </Button>
            </>
          )}
        </ItemList>
        <ErrorText message={listAction.error()} />
      </Section>

      <div ref={formTop}>
        <FormSection title="Export a folder" editing={exportForm.editing()?.path ?? null} onCancel={stopEditing}>
          <Show when={exportForm.form()} keyed>
            {({ editing }) => (
              <ExportForm initial={editing} busy={formAction.busy()} onSave={saveExport} onCancel={stopEditing} />
            )}
          </Show>
          <ErrorText message={formAction.error()} />
        </FormSection>
      </div>
    </>
  );
}

/** Adds an export, or edits `initial`. Starts from `initial`'s settings; rebuilt for each export. */
function ExportForm(props: {
  initial: NfsExport | null;
  busy: boolean;
  onSave: (nfsExport: NfsExport) => void;
  onCancel: () => void;
}) {
  const initial = props.initial;
  const [path, setPath] = createSignal(initial?.path ?? '');
  const [clients, setClients] = createSignal(initial?.clients.join(' ') ?? '');
  const [readOnly, setReadOnly] = createSignal(initial?.readOnly ?? false);

  const pickFolder = async () => {
    const folder = await chooseFolder();
    if (folder) setPath(folder);
  };

  /** The clients as typed: separated by spaces or commas. */
  const clientList = () =>
    clients()
      .split(/[\s,]+/)
      .filter(Boolean);

  const submit = (event: SubmitEvent) => {
    event.preventDefault();
    props.onSave({ path: path(), readOnly: readOnly(), clients: clientList() });
  };

  return (
    <form class="max-w-md space-y-4" onSubmit={submit}>
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

      <div class="flex gap-2">
        <Button type="submit" variant="primary" disabled={props.busy || !path() || clientList().length === 0}>
          {initial ? 'Save' : 'Export'}
        </Button>
        <Show when={initial}>
          <Button disabled={props.busy} onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
      </div>
    </form>
  );
}
