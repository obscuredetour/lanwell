// Reading share addresses as they're typed, before the helper checks them
// properly: enough to suggest a folder and to know which server to browse.

/** `//nas.lan/media`, `\\nas.lan\media` or `smb://nas.lan/media` → server and share. */
export function parseSmbAddress(address: string) {
  const rest = address
    .trim()
    .replace(/^(smb:\/\/|\/\/|\\\\)/, '')
    .replaceAll('\\', '/');
  const [host = '', share = ''] = rest.split('/');
  return { host, share };
}

/** `nas.lan:/srv/media` or `nfs://nas.lan/srv/media` → server and exported folder. */
export function parseNfsAddress(address: string) {
  const text = address.trim();
  if (text.startsWith('nfs://')) {
    const rest = text.slice('nfs://'.length);
    const slash = rest.indexOf('/');
    return slash < 0 ? { host: rest, path: '' } : { host: rest.slice(0, slash), path: rest.slice(slash) };
  }
  const colon = text.indexOf(':/');
  return colon < 0 ? { host: text, path: '' } : { host: text.slice(0, colon), path: text.slice(colon + 1) };
}

/** The last part of a path, as a folder name: `/srv/media/` → `media`. */
export function lastName(path: string) {
  return path.split('/').filter(Boolean).pop() ?? '';
}

/** `/home/alice/Network/media` as `~/Network/media`. */
export function tildify(path: string, home: string | null) {
  return home && path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}
