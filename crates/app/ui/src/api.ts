// The UI's only way into the backend: typed wrappers around the Rust commands
// in `crates/app/src/commands.rs`. The types come from `./bindings`, which
// ts-rs generates from the Rust structs (`cargo test` regenerates them).

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import type { HelperStatus } from './bindings/HelperStatus';
import type { NfsExport } from './bindings/NfsExport';
import type { Request } from './bindings/Request';
import type { SmbShare } from './bindings/SmbShare';
import type { State } from './bindings/State';
import type { SystemInfo } from './bindings/SystemInfo';

export type { HelperStatus, NfsExport, SmbShare, State, SystemInfo };

export const systemInfo = () => invoke<SystemInfo>('system_info');

/** Everything netfoo manages (read without a password). */
export const loadState = () => invoke<State>('load_state');

/** People with accounts on this machine. */
export const localUsers = () => invoke<string[]>('local_users');

/** Whether netfoo's privileged helper is installed, and the same version as the app. */
export const helperStatus = () => invoke<HelperStatus>('helper_status');

/** Installs or updates the helper. The system asks for an administrator's password. */
export const installHelper = () => invoke<void>('install_helper');

/** Removes the helper. The system asks for an administrator's password. */
export const uninstallHelper = () => invoke<void>('uninstall_helper');

// Changes. Each goes to the privileged helper, which may show a password
// prompt; they reject with a message to show as is.

const apply = (request: Request) => invoke<void>('apply', { request });

/** Replaces every Samba share netfoo manages. */
export const setSmbShares = (shares: SmbShare[]) => apply({ kind: 'setSmbShares', shares });

/** Replaces every NFS export netfoo manages. */
export const setNfsExports = (exports: NfsExport[]) => apply({ kind: 'setNfsExports', exports });

/** Sets a user's Samba password (separate from their login password). */
export const setSmbPassword = (user: string, password: string) => apply({ kind: 'setSmbPassword', user, password });

/** Asks for a folder; null if the person cancels. */
export const chooseFolder = async () => {
  const folder = await open({ directory: true });
  return typeof folder === 'string' ? folder : null;
};

/** The message from a rejected command. */
export const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));
