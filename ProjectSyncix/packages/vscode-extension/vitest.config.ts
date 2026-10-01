import { defineConfig } from 'vitest/config';
import * as path from 'path';

// The `vscode` module only exists inside the editor, so the tests get a fake one.
export default defineConfig({
    test: {
        environment: 'node',
        include: ['tests/**/*.test.ts'],
    },
    resolve: {
        alias: {
            vscode: path.resolve(__dirname, 'tests/vscode.mock.ts'),
        },
    },
});
