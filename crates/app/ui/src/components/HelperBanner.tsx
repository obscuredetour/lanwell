// Offers to install lanwell's privileged helper when it's missing or from
// another version. Without it lanwell can show what's shared but not change it.

import { Match, Switch } from 'solid-js';

import { type HelperStatus, installHelper } from '../api';
import { Button, createAction, ErrorText } from './ui';

export function HelperBanner(props: { status: HelperStatus | undefined; onChanged: () => void }) {
  const action = createAction();

  const install = async () => {
    if (await action.run(installHelper)) props.onChanged();
  };

  const Banner = (banner: { message: string; button: string }) => (
    <div class="mb-6 rounded-md border border-accent px-4 py-3">
      <div class="flex items-center gap-4">
        <p class="flex-1">{banner.message}</p>
        <Button variant="primary" disabled={action.busy()} onClick={install}>
          {banner.button}
        </Button>
      </div>
      <ErrorText message={action.error()} />
    </div>
  );

  return (
    <Switch>
      <Match when={props.status?.kind === 'missing'}>
        <Banner
          message="To change what's shared, lanwell needs a small system helper. Installing it asks for an administrator's password, once."
          button="Install helper"
        />
      </Match>
      <Match when={props.status?.kind === 'outdated'}>
        <Banner
          message="lanwell's system helper is from a different build of lanwell. Update it to keep changes working."
          button="Update helper"
        />
      </Match>
    </Switch>
  );
}
