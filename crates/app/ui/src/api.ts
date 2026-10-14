// The UI's only way into the backend: typed wrappers around the Rust commands
// in `crates/app/src/commands.rs`. The types come from `./bindings`, which
// ts-rs generates from the Rust structs (`cargo test` regenerates them).

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import type { FileServer } from './bindings/FileServer';
import type { FileServerStatus } from './bindings/FileServerStatus';
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
import type { UpdateStatus } from './bindings/UpdateStatus';

export type {
  FileServer,
  FileServerStatus,
  FoundServer,
  HelperStatus,
  ManagedMount,
  Mount,
  MountStatus,
  NfsExport,
  SmbShare,
  State,
  SystemInfo,
  UpdateStatus,
};

export const systemInfo = () => invoke<SystemInfo>('system_info');

/** Everything lanwell manages (read without a password). */
export const loadState = () => invoke<State>('load_state');

/** Whether Samba or the NFS server is installed, running, and let through the firewall. */
export const fileServerStatus = (server: FileServer) => invoke<FileServerStatus>('file_server_status', { server });

/** People with accounts on this machine. */
export const localUsers = () => invoke<string[]>('local_users');

/** Whether lanwell's privileged helper is installed, and the same version as the app. */
export const helperStatus = () => invoke<HelperStatus>('helper_status');

/** Installs or updates the helper. The system asks for an administrator's password. */
export const installHelper = () => invoke<void>('install_helper');

/** Removes the helper. The system asks for an administrator's password. */
export const uninstallHelper = () => invoke<void>('uninstall_helper');

// Changes. Each goes to the privileged helper, which may show a password
// prompt; they reject with a message to show as is.

const apply = (request: Request) => invoke<void>('apply', { request });

/** Replaces every Samba share lanwell manages. */
export const setSmbShares = (shares: SmbShare[]) => apply({ kind: 'setSmbShares', shares });

/** Replaces every NFS export lanwell manages. */
export const setNfsExports = (exports: NfsExport[]) => apply({ kind: 'setNfsExports', exports });

/** Starts a file server and has it start with the computer, or stops it and has it not. */
export const setServerRunning = (server: FileServer, running: boolean) =>
  apply({ kind: 'setServerRunning', server, running });

/** Lets a file server through the firewall from the local network, or takes back what lanwell let through. */
export const setFirewall = (server: FileServer, allow: boolean) => apply({ kind: 'setFirewall', server, allow });

/** Sets a user's Samba password (separate from their login password). */
export const setSmbPassword = (user: string, password: string) => apply({ kind: 'setSmbPassword', user, password });

/**
 * Mounts a share from another machine, SMB or NFS (replacing any lanwell mount
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

// Updates. lanwell only says a new version is out; install.sh installs it.

/** Whether a newer lanwell is out (only asks GitHub if install.sh installed this copy and checking is on). */
export const updateStatus = () => invoke<UpdateStatus>('update_status');

/** Turns the check for new versions (at launch and once a day) on or off. */
export const setUpdateChecks = (on: boolean) => invoke<void>('set_update_checks', { on });

/** Opens the latest release's notes in the browser. */
export const openReleaseNotes = () => invoke<void>('open_release_notes');

/** Asks for a folder; null if the person cancels. */
export const chooseFolder = async () => {
  const folder = await open({ directory: true });
  return typeof folder === 'string' ? folder : null;
};

/** The message from a rejected command. */
export const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));
