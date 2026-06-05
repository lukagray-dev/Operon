'use strict';

import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverModels, saveProviderSetup } from './ipc.js';

/**
 * Install a tiny Tauri mock so we can verify the exact payload that the IPC
 * helper sends to the backend without needing the desktop shell to boot.
 *
 * @param {(calls: Array<{ command: string, args: Object }>) => Promise<unknown>} run - Test body that receives the recorded invocations.
 * @returns {Promise<unknown>}
 */
async function withTauriMock(run) {
    const originalWindow = globalThis.window;
    const calls = [];

    globalThis.window = {
        __TAURI__: {
            core: {
                invoke: async (command, args) => {
                    calls.push({ command, args });
                    return { command, args };
                },
            },
        },
    };

    try {
        return await run(calls);
    } finally {
        if (typeof originalWindow === 'undefined') {
            delete globalThis.window;
        } else {
            globalThis.window = originalWindow;
        }
    }
}

test('ipc model provider helpers wrap request payloads correctly', async (t) => {
    await t.test('discoverModels sends a nested request object', async () => {
        await withTauriMock(async (calls) => {
            const response = await discoverModels({
                providerId: ' groq ',
                apiBase: ' https://api.groq.com/openai/v1 ',
                apiKey: ' gsk_test_key ',
            });

            assert.equal(calls.length, 1);
            assert.deepEqual(calls[0], {
                command: 'discover_models',
                args: {
                    request: {
                        providerId: 'groq',
                        apiBase: 'https://api.groq.com/openai/v1',
                        apiKey: 'gsk_test_key',
                    },
                },
            });
            assert.deepEqual(response, calls[0]);
        });
    });

    await t.test('saveProviderSetup sends a nested request object', async () => {
        await withTauriMock(async (calls) => {
            const response = await saveProviderSetup({
                providerId: ' open_ai ',
                apiBase: ' https://api.openai.com/v1 ',
                apiKey: ' sk-test ',
                model: ' gpt-4o-mini ',
            });

            assert.equal(calls.length, 1);
            assert.deepEqual(calls[0], {
                command: 'save_provider_setup',
                args: {
                    request: {
                        providerId: 'open_ai',
                        apiBase: 'https://api.openai.com/v1',
                        apiKey: 'sk-test',
                        model: 'gpt-4o-mini',
                    },
                },
            });
            assert.deepEqual(response, calls[0]);
        });
    });
});
