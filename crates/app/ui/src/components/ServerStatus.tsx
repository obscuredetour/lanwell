// The top of the Samba and NFS pages: whether the server is running, and
// whether the firewall lets other computers reach it, with what to do about each.

import type { JSX } from 'solid-js';
import { createResource, Match, Show, Switch } from 'solid-js';

import { type FileServer, type FileServerStatus, fileServerStatus, setFirewall, setServerRunning } from '../api';
import { Button, createAction, ErrorText } from './ui';

/** How each is named starting a sentence, and in the middle of one. */
const NAMES: Record<FileServer, { start: string; within: string }> = {
  samba: { start: 'Samba', within: 'Samba' },
  nfs: { start: 'The NFS server', within: 'the NFS server' },
};
const FIREWALLS = { none: '', ufw: 'ufw', firewalld: 'firewalld' } as const;

export function ServerStatus(props: {
  server: FileServer;
  /** How many folders are shared; the status is checked again when it changes (sharing starts the server). */
  shared: number;
}) {
  const [status, { refetch }] = createResource(
    () => ({ server: props.server, shared: props.shared }),
    ({ server }) => fileServerStatus(server),
  );
  const action = createAction();

  const run = async (change: () => Promise<void>) => {
    await action.run(change);
    refetch();
  };

  const name = () => NAMES[props.server].start;

  return (
    <Show when={status()?.installed ? status() : null}>
      {(current) => (
        <div class="mb-6 space-y-2 rounded-md border border-border px-4 py-3">
          <Line ok={current().running && current().enabled}>
            <Switch>
              <Match when={current().running && current().enabled}>
                {name()} is running, and starts with the computer.
              </Match>
              <Match when={current().running}>{name()} is running, but won't start with the computer.</Match>
              <Match when={true}>{name()} isn't running, so nothing here is shared.</Match>
            </Switch>
            <Show
              when={current().running && current().enabled}
              fallback={
                <Button
                  variant="primary"
                  disabled={action.busy()}
                  onClick={() => run(() => setServerRunning(props.server, true))}
                >
                  {current().running ? 'Start with the computer' : 'Start'}
                </Button>
              }
            >
              <Button disabled={action.busy()} onClick={() => run(() => setServerRunning(props.server, false))}>
                Stop
              </Button>
            </Show>
          </Line>

          <FirewallLine
            status={current()}
            name={NAMES[props.server].within}
            busy={action.busy()}
            onChange={(allow) => run(() => setFirewall(props.server, allow))}
          />
          <ErrorText message={action.error()} />
        </div>
      )}
    </Show>
  );
}

function FirewallLine(props: {
  status: FileServerStatus;
  name: string;
  busy: boolean;
  onChange: (allow: boolean) => void;
}) {
  const firewall = () => `The firewall (${FIREWALLS[props.status.firewall]})`;
  const access = () => props.status.access;
  /** The access, if the firewall lets the server in. */
  const allowed = () => {
    const current = access();
    return current.kind === 'allowed' ? current : null;
  };

  return (
    <Switch>
      <Match when={props.status.firewall === 'none'}>
        <Line ok>No firewall is on, so nothing blocks it.</Line>
      </Match>
      <Match when={allowed()}>
        {(allowed) => (
          <Line ok>
            {allowed().byNetfoo
              ? `${firewall()} lets ${props.name} in from your local network.`
              : `${firewall()} lets ${props.name} in.`}
            <Show when={allowed().byNetfoo}>
              <Button disabled={props.busy} onClick={() => props.onChange(false)}>
                Stop allowing
              </Button>
            </Show>
          </Line>
        )}
      </Match>
      <Match when={true}>
        <Line ok={false}>
          {access().kind === 'blocked'
            ? `${firewall()} blocks ${props.name}: other computers can't connect.`
            : `${firewall()} may be blocking ${props.name}; its rules can't be read without an administrator's password.`}
          <Button variant="primary" disabled={props.busy} onClick={() => props.onChange(true)}>
            Allow from local network
          </Button>
        </Line>
      </Match>
    </Switch>
  );
}

/** A status sentence with a dot (green-ish when all's well), and an action at the end. */
function Line(props: { ok: boolean; children: JSX.Element }) {
  return (
    <div class="flex items-center gap-3">
      <span class="size-2 shrink-0 rounded-full" classList={{ 'bg-accent': props.ok, 'bg-warning': !props.ok }} />
      <div class="flex flex-1 items-center justify-between gap-3">{props.children}</div>
    </div>
  );
}
