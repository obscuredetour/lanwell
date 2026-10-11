// The UI's only way into the backend: typed wrappers around the Rust commands
// in `crates/app/src/commands.rs`. The types come from `./bindings`, which
// ts-rs generates from the Rust structs (`cargo test` regenerates them).

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import type { FoundServer } from './bindings/FoundServer';
import type { HelperStatus } from './bindings/HelperStatus';
import type { ManagedMount } from './bindings/ManagedMount';
import type { Mount } from './bindings/Mount';
import type { MountStatus } from './bindings/MountStatus';
import type { NfsExport } from './bindings/NfsExport';
import type { Request } from './bindings/Request';
import type { SmbShare } from './bindings/SmbShare';
import type { State } from './bindings/State';
import type { SystemInfo } from './bindings/SystemInfo';

export type { FoundServer, HelperStatus, ManagedMount, Mount, MountStatus, NfsExport, SmbShare, State, SystemInfo };

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

/**
 * Mounts a share from another machine, SMB or NFS (replacing any netfoo mount
 * on the same folder). `password` is for an SMB user login: a new one needs
 * it; null keeps the saved one. `replaces` is the folder of the mount being
 * edited, if the edit moves it.
 */
export const setMount = (mount: Mount, password: string | null, replaces: string | null = null) =>
  apply({ kind: 'setMount', mount, password, replaces });

/** Unmounts the share on `mountPoint` and stops mounting it. The folder stays. */
export const removeMount = (mountPoint: string) => apply({ kind: 'removeMount', mountPoint });

// Mounts, read without a password.

/** What each mount is doing now: mounted, waiting to be opened, or failed. */
export const mountStatuses = () => invoke<MountStatus[]>('mount_statuses');

/** The shares on a server. Signs in as `username` if given, or as a guest. */
export const listServerShares = (host: string, username: string | null, password: string | null) =>
  invoke<string[]>('list_server_shares', { host, username, password });

/** File servers that announce themselves on the local network (takes a few seconds). */
export const findServers = () => invoke<FoundServer[]>('find_servers');

/** The folders an NFS server exports. */
export const listServerExports = (host: string) => invoke<string[]>('list_server_exports', { host });

/** Opens a mount's folder in the file manager (mounting it if needed). */
export const openMount = (mountPoint: string) => invoke<void>('open_mount', { mountPoint });

/** Asks for a folder; null if the person cancels. */
export const chooseFolder = async () => {
  const folder = await open({ directory: true });
  return typeof folder === 'string' ? folder : null;
};

/** The message from a rejected command. */
export const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));
