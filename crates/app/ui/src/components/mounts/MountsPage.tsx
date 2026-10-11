// Shares on other computers, mounted as folders on this one: the list of
// mounts, and the form to add one (ConnectForm).

import { createResource, createSignal, onCleanup } from 'solid-js';

import { type ManagedMount, type MountStatus, mountStatuses, openMount, removeMount, type SystemInfo } from '../../api';
import { Button, createAction, ErrorText, ItemList, Section, Tag } from '../ui';
import { tildify } from './addresses';
import { ConnectForm } from './ConnectForm';

/** How often the list checks what each mount is doing. */
const STATUS_REFRESH_MS = 5000;

export function MountsPage(props: { info: SystemInfo; mounts: ManagedMount[]; onChanged: () => void }) {
  const [statuses, { refetch: refetchStatuses }] = createResource(mountStatuses, { initialValue: [] });
  const timer = setInterval(refetchStatuses, STATUS_REFRESH_MS);
  onCleanup(() => clearInterval(timer));

  const action = createAction();
  /** The mount being edited in the form below, if any. */
  const [editing, setEditing] = createSignal<ManagedMount | null>(null);
  let form: HTMLDivElement | undefined;

  const startEditing = (managed: ManagedMount) => {
    setEditing(managed);
    form?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  const changed = () => {
    props.onChanged();
    refetchStatuses();
  };

  const remove = async (mountPoint: string) => {
    if (!(await action.run(() => removeMount(mountPoint)))) return;
    if (editing()?.mountPoint === mountPoint) setEditing(null);
    changed();
  };

  const statusOf = (mountPoint: string) => statuses().find((status) => status.mountPoint === mountPoint);

  return (
    <>
      <Section title="Network folders" description="Shares on other computers, as folders on this one.">
        <ItemList items={props.mounts} empty="No shares are mounted yet.">
          {(managed) => (
            <>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="font-medium">{managed.share.address}</span>
                  <Tag>{managed.share.protocol.toUpperCase()}</Tag>
                </div>
                <div class="truncate font-mono text-xs text-fg-muted">
                  {tildify(managed.mountPoint, props.info.homeDir)}
                </div>
              </div>
              <StatusLabel status={statusOf(managed.mountPoint)} />
              <Button disabled={action.busy()} onClick={() => action.run(() => openMount(managed.mountPoint))}>
                Open
              </Button>
              <Button disabled={action.busy()} onClick={() => startEditing(managed)}>
                Edit
              </Button>
              <Button variant="danger" disabled={action.busy()} onClick={() => remove(managed.mountPoint)}>
                Remove
              </Button>
            </>
          )}
        </ItemList>
        <ErrorText message={action.error()} />
      </Section>

      <div ref={form}>
        <Section title={editing() ? `Edit ${editing()!.share.address}` : 'Connect to a share'}>
          <ConnectForm info={props.info} editing={editing()} onChanged={changed} onDone={() => setEditing(null)} />
        </Section>
      </div>
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
