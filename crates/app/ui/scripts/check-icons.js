// Fails if an icon is imported from anywhere but src/icons.ts, or if icons.ts
// imports the whole of lucide-solid (see the comment in icons.ts). Part of `pnpm lint`.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const iconsFile = join('src', 'icons.ts');
const problems = [];

for (const name of readdirSync('src', { recursive: true })) {
  const file = join('src', name);
  if (!/\.(ts|tsx)$/.test(file)) continue;
  const text = readFileSync(file, 'utf8');
  if (file === iconsFile) {
    if (/^\s*(import|export)\b[^;]*from\s+['"]lucide-solid['"]/m.test(text))
      problems.push(`${file}: import each icon from 'lucide-solid/icons/<name>'`);
  } else if (text.includes('lucide-solid')) {
    problems.push(`${file}: import icons from './icons' (add new ones to ${iconsFile})`);
  }
}

if (problems.length > 0) {
  console.error(problems.join('\n'));
  process.exit(1);
}
