#!/usr/bin/env node

/**
 * Synchronizes the version in package.json to every other version-bearing file
 * in the repo, and — with `--check` — asserts they already agree.
 *
 * Run as part of `changeset version`, so the synced manifests are COMMITTED with
 * the version bump. It used to run after `changeset publish` instead, which
 * mutated the CI workspace and threw the result away: every git tag shipped the
 * wrong version constants (`git show go/fetch/v3.3.10:go/fetch/version.go` said
 * 2.1.2 while package.json said 3.3.10), and `cargo publish --allow-dirty`
 * existed only to paper over the resulting dirt.
 *
 * `--check` is the guard that keeps that from coming back. It fails loudly —
 * exit 1, naming each file and both versions — and it also fails when a pattern
 * stops matching, because a guard that finds nothing to check must not report
 * success.
 */

import { readFileSync, writeFileSync } from 'fs';
import { dirname, join, relative } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = join(__dirname, '..');
const check = process.argv.includes('--check');

const packageJson = JSON.parse(readFileSync(join(rootDir, 'package.json'), 'utf8'));
const version = packageJson.version;
const major = Number(version.split('.')[0]);

if (!Number.isInteger(major)) {
    console.error(`package.json version "${version}" has no integer major component.`);
    process.exit(1);
}

// Go requires a `/vN` module-path suffix for major >= 2, and NO suffix below it.
// Without it `go get github.com/SmooAI/fetch/go/fetch@v3.3.10` resolves nothing —
// which is exactly what every `go/fetch/v3.x` tag did until this was added.
const GO_MODULE_BASE = 'github.com/SmooAI/fetch/go/fetch';
const goModulePath = major >= 2 ? `${GO_MODULE_BASE}/v${major}` : GO_MODULE_BASE;

const files = [
    {
        path: join(rootDir, 'python', 'pyproject.toml'),
        pattern: /^version = ".*"$/m,
        replacement: `version = "${version}"`,
    },
    {
        // `smooai_fetch.__version__` is a plain literal, not read from package
        // metadata — so PyPI could ship 3.4.0 while the module reported 2.1.2.
        path: join(rootDir, 'python', 'src', 'smooai_fetch', '__init__.py'),
        pattern: /^__version__ = ".*"$/m,
        replacement: `__version__ = "${version}"`,
    },
    {
        path: join(rootDir, 'rust', 'fetch', 'Cargo.toml'),
        pattern: /^version = ".*"$/m,
        replacement: `version = "${version}"`,
    },
    {
        // Keep rust/fetch/Cargo.lock's own crate entry in lockstep with the Cargo.toml
        // bump above — name-targeted so a same-versioned DEPENDENCY is never touched.
        // Without this the lock pins the old version and `cargo build/publish --locked`
        // rejects the mismatch (which is why the release used `--allow-dirty`); stamping
        // it lets the publish run `--locked` reproducibly.
        path: join(rootDir, 'rust', 'fetch', 'Cargo.lock'),
        pattern: /(name = "smooai-fetch"\nversion = )"[^"]*"/,
        replacement: `$1"${version}"`,
    },
    {
        path: join(rootDir, 'go', 'fetch', 'version.go'),
        pattern: /const Version = ".*"/,
        replacement: `const Version = "${version}"`,
    },
    {
        // The module path's major suffix, not a version string — but it is derived
        // from the same number, so it belongs in the same sync and the same guard.
        path: join(rootDir, 'go', 'fetch', 'go.mod'),
        pattern: /^module \S+$/m,
        replacement: `module ${goModulePath}`,
    },
    {
        path: join(rootDir, 'dotnet', 'SmooAI.Fetch', 'SmooAI.Fetch.csproj'),
        pattern: /<Version>.*<\/Version>/,
        replacement: `<Version>${version}</Version>`,
    },
];

const problems = [];

for (const file of files) {
    const rel = relative(rootDir, file.path);
    let content;
    try {
        content = readFileSync(file.path, 'utf8');
    } catch (error) {
        if (error.code !== 'ENOENT') throw error;
        problems.push(`${rel}: expected file is missing`);
        continue;
    }

    // A pattern that no longer matches would make `replace` a no-op, which reads
    // as "already up to date". Treat it as a failure in both modes.
    if (!file.pattern.test(content)) {
        problems.push(`${rel}: pattern ${file.pattern} no longer matches — the sync would silently do nothing`);
        continue;
    }

    const updated = content.replace(file.pattern, file.replacement);
    if (content === updated) {
        if (!check) console.log(`  Already up to date: ${rel}`);
        continue;
    }

    if (check) {
        problems.push(`${rel}: out of sync with package.json version ${version} (expected \`${file.replacement}\`)`);
    } else {
        writeFileSync(file.path, updated);
        console.log(`  Updated ${rel}`);
    }
}

if (problems.length > 0) {
    console.error(`\nVersion consistency check FAILED against package.json ${version}:\n`);
    for (const problem of problems) console.error(`  ✗ ${problem}`);
    console.error(`\nRun \`node scripts/sync-versions.mjs\` and commit the result.\n`);
    process.exit(1);
}

console.log(check ? `All manifests agree with package.json ${version}.` : 'Done!');
