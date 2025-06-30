import { fileURLToPath } from 'url';
import alias from 'esbuild-plugin-alias';
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
    entry: ['src/index.ts'],
    outDir: 'dist/browser',
    platform: 'browser',
    dts: true,
    noExternal: ['@smooai/logger/Logger', '@smooai/logger/AwsServerLogger'],
    esbuildPlugins: [
        alias({
            '@smooai/logger/AwsServerLogger': fileURLToPath(import.meta.resolve('@smooai/logger/browser/BrowserLogger')),
            '@smooai/logger/Logger': fileURLToPath(import.meta.resolve('@smooai/logger/browser/Logger')),
        }),
    ],
};

export default defineConfig([coreConfig, browserConfig]);
