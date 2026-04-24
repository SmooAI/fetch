#!/usr/bin/env node

/**
 * Synchronizes version from package.json to all sub-package manifests.
 */

import { readFileSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = join(__dirname, '..');

const packageJson = JSON.parse(readFileSync(join(rootDir, 'package.json'), 'utf8'));
const version = packageJson.version;

console.log(`Syncing version ${version} to all sub-packages...`);

const files = [
    {
        path: join(rootDir, 'python', 'pyproject.toml'),
        pattern: /^version = ".*"$/m,
        replacement: `version = "${version}"`,
    },
    {
        path: join(rootDir, 'rust', 'fetch', 'Cargo.toml'),
        pattern: /^version = ".*"$/m,
        replacement: `version = "${version}"`,
    },
    {
        path: join(rootDir, 'go', 'fetch', 'version.go'),
        pattern: /const Version = ".*"/,
        replacement: `const Version = "${version}"`,
    },
    {
        path: join(rootDir, 'dotnet', 'SmooAI.Fetch', 'SmooAI.Fetch.csproj'),
        pattern: /<Version>.*<\/Version>/,
        replacement: `<Version>${version}</Version>`,
    },
];

for (const file of files) {
    try {
        const content = readFileSync(file.path, 'utf8');
        const updated = content.replace(file.pattern, file.replacement);
        if (content !== updated) {
            writeFileSync(file.path, updated);
            console.log(`  Updated ${file.path}`);
        } else {
            console.log(`  Already up to date: ${file.path}`);
        }
    } catch (error) {
        if (error.code === 'ENOENT') {
            console.log(`  Skipped (not found): ${file.path}`);
        } else {
            throw error;
        }
    }
}

console.log('Done!');
