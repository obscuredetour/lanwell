// Every icon the app uses. This is the only file that imports lucide-solid
// (`pnpm lint` checks), so the set stays small and in one place: add an icon
// here, then import it from './icons'.
//
// Each comes from its own file, never from 'lucide-solid' itself: in
// development that loads all ~1,700 icons as separate modules, which runs
// WebKit out of file descriptors ("Failed to create shared memory").
// Names: https://lucide.dev/icons

export { default as FolderOpen } from 'lucide-solid/icons/folder-open';
export { default as Network } from 'lucide-solid/icons/network';
export { default as ShieldCheck } from 'lucide-solid/icons/shield-check';
export { default as HardDrive } from 'lucide-solid/icons/hard-drive';
