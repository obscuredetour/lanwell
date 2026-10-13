// Folders shared over SMB, and the passwords people use to open them.

import { createResource, createSignal, For, Show } from 'solid-js';

import { chooseFolder, localUsers, setSmbPassword, setSmbShares, type SmbShare, type SystemInfo } from '../api';
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

export function SambaPage(props: { info: SystemInfo; shares: SmbShare[]; onChanged: () => void }) {
  const [users] = createResource(localUsers, { initialValue: [] });
  // Separate, so an error shows next to what caused it.
  const listAction = createAction();
  const formAction = createAction();
  const shareForm = createEditing<SmbShare>();
  let formTop: HTMLDivElement | undefined;

  /** Saves the whole list (the helper replaces what it had with it). */
  const save = async (action: ReturnType<typeof createAction>, shares: SmbShare[]) => {
    const saved = await action.run(() => setSmbShares(shares));
    if (saved) props.onChanged();
    return saved;
  };

  /** A new share, or the one being edited (found by its name, which is unique) changed. */
  const saveShare = async (share: SmbShare) => {
    const original = shareForm.editing();
    const shares = original
      ? props.shares.map((other) => (other.name === original.name ? share : other))
      : [...props.shares, share];
    if (await save(formAction, shares)) shareForm.done();
  };

  const stopEditing = () => {
    formAction.reset();
    shareForm.done();
  };

  /** Whether `share` is the one the form below is editing. */
  const isEditing = (share: SmbShare) => shareForm.editing()?.name === share.name;

  const startEditing = (share: SmbShare) => {
    formAction.reset();
    shareForm.edit(share);
    formTop?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  return (
    <>
      <Show when={!props.info.sambaInstalled}>
        <Notice>
          Samba isn't installed. Install your distribution's <code>samba</code> package to share folders with Windows,
          Macs and phones.
        </Notice>
      </Show>

      <ServerStatus server="samba" shared={props.shares.length} />

      <Section title="Shared folders" description="Folders other computers can open over the network.">
        <ItemList items={props.shares} empty="No folders are shared yet." highlighted={isEditing}>
          {(share) => (
            <>
              <div class="min-w-0 flex-1">
                <div class="font-medium">{share.name}</div>
                <div class="truncate font-mono text-xs text-fg-muted">{share.path}</div>
              </div>
              <span class="text-xs text-fg-muted">
                {share.access.kind === 'guest' ? 'Anyone' : share.access.users.join(', ')}
                {share.readOnly ? ' · read only' : ''}
              </span>
              <Button disabled={listAction.busy() || isEditing(share)} onClick={() => startEditing(share)}>
                {isEditing(share) ? 'Editing' : 'Edit'}
              </Button>
              <Button
                variant="danger"
                disabled={listAction.busy()}
                onClick={() =>
                  save(
                    listAction,
                    props.shares.filter((other) => other !== share),
                  )
                }
              >
                Stop sharing
              </Button>
            </>
          )}
        </ItemList>
        <ErrorText message={listAction.error()} />
      </Section>

      <div ref={formTop}>
        <FormSection
          title="Share a folder"
          editing={shareForm.editing() ? `“${shareForm.editing()!.name}”` : null}
          onCancel={stopEditing}
        >
          <Show when={shareForm.form()} keyed>
            {({ editing }) => (
              <ShareForm
                users={users()}
                initial={editing}
                busy={formAction.busy()}
                onSave={saveShare}
                onCancel={stopEditing}
              />
            )}
          </Show>
          <ErrorText message={formAction.error()} />
        </FormSection>
      </div>

      <Section
        title="Passwords"
        description="Samba keeps its own passwords, separate from the ones people log in with. Set one for each person a folder is shared with."
      >
        <PasswordForm users={users()} />
      </Section>
    </>
  );
}

/** Adds a share, or edits `initial`. Starts from `initial`'s settings; rebuilt for each share. */
function ShareForm(props: {
  users: string[];
  initial: SmbShare | null;
  busy: boolean;
  onSave: (share: SmbShare) => void;
  onCancel: () => void;
}) {
  const initial = props.initial;
  const [path, setPath] = createSignal(initial?.path ?? '');
  const [name, setName] = createSignal(initial?.name ?? '');
  const [guest, setGuest] = createSignal(initial?.access.kind === 'guest');
  const [chosenUsers, setChosenUsers] = createSignal<string[]>(
    initial?.access.kind === 'users' ? initial.access.users : [],
  );
  const [readOnly, setReadOnly] = createSignal(initial?.readOnly ?? false);

  const pickFolder = async () => {
    const folder = await chooseFolder();
    if (!folder) return;
    setPath(folder);
    // The folder's own name is usually the right name for the share.
    if (!name()) setName(folder.split('/').filter(Boolean).pop() ?? '');
  };

  const toggleUser = (user: string, on: boolean) =>
    setChosenUsers((users) => (on ? [...users, user] : users.filter((other) => other !== user)));

  const submit = (event: SubmitEvent) => {
    event.preventDefault();
    props.onSave({
      name: name().trim(),
      path: path(),
      readOnly: readOnly(),
      access: guest() ? { kind: 'guest' } : { kind: 'users', users: chosenUsers() },
    });
  };

  return (
    <form class="max-w-md space-y-4" onSubmit={submit}>
      <Field label="Folder">
        <div class="flex gap-2">
          <input class={inputClass} value={path()} onInput={(e) => setPath(e.currentTarget.value)} />
          <Button onClick={pickFolder}>Choose…</Button>
        </div>
      </Field>

      <Field label="Name" hint="What other computers see it as.">
        <input class={inputClass} value={name()} onInput={(e) => setName(e.currentTarget.value)} />
      </Field>

      <fieldset class="space-y-1.5">
        <legend class="mb-1 font-medium">Who can open it</legend>
        <label class="flex items-center gap-2">
          <input type="radio" checked={!guest()} onChange={() => setGuest(false)} />
          Only these people, with their Samba password:
        </label>
        <div class="space-y-1 pl-6">
          <For each={props.users} fallback={<p class="text-fg-muted">No user accounts found.</p>}>
            {(user) => (
              <label class="flex items-center gap-2">
                <input
                  type="checkbox"
                  disabled={guest()}
                  checked={chosenUsers().includes(user)}
                  onChange={(e) => toggleUser(user, e.currentTarget.checked)}
                />
                {user}
              </label>
            )}
          </For>
        </div>
        <label class="flex items-center gap-2">
          <input type="radio" checked={guest()} onChange={() => setGuest(true)} />
          Anyone on the network, without a password
        </label>
      </fieldset>

      <label class="flex items-center gap-2">
        <input type="checkbox" checked={readOnly()} onChange={(e) => setReadOnly(e.currentTarget.checked)} />
        Read only
      </label>

      <div class="flex gap-2">
        <Button type="submit" variant="primary" disabled={props.busy || !path() || !name().trim()}>
          {initial ? 'Save' : 'Share'}
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

function PasswordForm(props: { users: string[] }) {
  const [user, setUser] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [confirmation, setConfirmation] = createSignal('');
  const [done, setDone] = createSignal(false);
  const action = createAction();

  const mismatch = () => confirmation() !== '' && confirmation() !== password();

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    setDone(false);
    if (await action.run(() => setSmbPassword(user(), password()))) {
      setPassword('');
      setConfirmation('');
      setDone(true);
    }
  };

  return (
    <form class="max-w-md space-y-4" onSubmit={submit}>
      <Field label="Person">
        <select class={inputClass} value={user()} onChange={(e) => setUser(e.currentTarget.value)}>
          <option value="" disabled>
            Choose…
          </option>
          <For each={props.users}>{(name) => <option value={name}>{name}</option>}</For>
        </select>
      </Field>
      <Field label="Password">
        <input
          type="password"
          class={inputClass}
          value={password()}
          onInput={(e) => setPassword(e.currentTarget.value)}
        />
      </Field>
      <Field label="Password again">
        <input
          type="password"
          class={inputClass}
          value={confirmation()}
          onInput={(e) => setConfirmation(e.currentTarget.value)}
        />
      </Field>
      <Show when={mismatch()}>
        <p class="text-danger">The passwords don't match.</p>
      </Show>
      <Button
        type="submit"
        variant="primary"
        disabled={action.busy() || !user() || !password() || password() !== confirmation()}
      >
        Set password
      </Button>
      <ErrorText message={action.error()} />
      <Show when={done()}>
        <p class="text-accent">Password set for {user()}.</p>
      </Show>
    </form>
  );
}
