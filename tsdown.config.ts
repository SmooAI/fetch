import { createRequire } from 'module';
import alias from '@rollup/plugin-alias';
import { defineConfig, type Options } from 'tsdown';

const require_ = createRequire(import.meta.url);

const coreConfig: Options = {
    entry: ['src/index.ts'],
    clean: true,
    dts: true,
    format: ['cjs', 'esm'],
    sourcemap: true,
    target: 'es2022',
    treeshake: true,
};

const browserConfig: Options = {
    ...coreConfig,
    entry: ['src/index.ts'],
    outDir: 'dist/browser',
    platform: 'browser',
    fixedExtension: true,
    dts: true,
    // `@smooai/logger` Node entries are aliased to browser variants below
    // and bundled (kept out of `external`).
    deps: {
        alwaysBundle: ['@smooai/logger/Logger', '@smooai/logger/AwsServerLogger'],
    },
    plugins: [
        alias({
            entries: [
                { find: '@smooai/logger/AwsServerLogger', replacement: require_.resolve('@smooai/logger/browser/BrowserLogger') },
                { find: '@smooai/logger/Logger', replacement: require_.resolve('@smooai/logger/browser/Logger') },
            ],
        }),
    ],
};

export default defineConfig([coreConfig, browserConfig]);
