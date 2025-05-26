import { defineConfig, type Options } from 'tsup';

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
    entry: ['src/browser.ts'],
    platform: 'browser',
    dts: true,
};

export default defineConfig([coreConfig, browserConfig]);
