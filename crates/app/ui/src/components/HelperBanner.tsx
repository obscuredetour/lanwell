// Says when lanwell's privileged helper is missing or from another version.
// Without it lanwell can show what's shared but not change it. The app
// installs it only from a helper beside it that nothing else could have
// changed (in development, or from a package); otherwise the installer does.

import { Match, Switch } from 'solid-js';

import { type HelperStatus, installHelper } from '../api';
import { Button, createAction, ErrorText, InstallCommand } from './ui';

export function HelperBanner(props: { status: HelperStatus | undefined; onChanged: () => void }) {
  const action = createAction();

  const install = async () => {
    if (await action.run(installHelper)) props.onChanged();
  };

  const installable = () =>
    (props.status?.kind === 'missing' || props.status?.kind === 'outdated') && props.status.installable;

  const Banner = (banner: { message: string; button: string; fromInstaller: string }) => (
    <div class="mb-6 rounded-md border border-accent px-4 py-3">
      <Switch>
        <Match when={installable()}>
          <div class="flex items-center gap-4">
            <p class="flex-1">{banner.message}</p>
            <Button variant="primary" disabled={action.busy()} onClick={install}>
              {banner.button}
            </Button>
          </div>
        </Match>
        <Match when={!installable()}>
          <div class="flex items-start gap-4">
            <div class="min-w-0 flex-1">
              <p>{banner.fromInstaller}</p>
              <InstallCommand />
            </div>
            <Button class="shrink-0" onClick={() => props.onChanged()}>
              Check again
            </Button>
          </div>
        </Match>
      </Switch>
      <ErrorText message={action.error()} />
    </div>
  );

  return (
    <Switch>
      <Match when={props.status?.kind === 'missing'}>
        <Banner
          message="To change what's shared, Lanwell needs a small system helper. Installing it asks for an administrator's password, once."
          button="Install helper"
          fromInstaller="To change what's shared, Lanwell needs a small system helper. To install it, run the installer again (it asks for your password):"
        />
      </Match>
      <Match when={props.status?.kind === 'outdated'}>
        <Banner
          message="Lanwell's system helper is from a different build of Lanwell. Update it to keep changes working."
          button="Update helper"
          fromInstaller="Lanwell's system helper is from a different version of Lanwell. To update it, run the installer again (it asks for your password):"
        />
      </Match>
    </Switch>
  );
}
