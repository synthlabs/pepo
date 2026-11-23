import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const makeVersion = spawnSync('make', ['--version'], { encoding: 'utf8' });
const makeOptions = {
	skip: /GNU Make/.test(makeVersion.stdout ?? '') ? false : 'GNU Make is not available'
};

for (const scenario of [
	{ name: 'default', inherited: undefined, override: undefined, expected: 'debug' },
	{ name: 'inherited preference', inherited: 'info', override: undefined, expected: 'info' },
	{ name: 'explicit override', inherited: 'info', override: 'trace', expected: 'trace' }
]) {
	test(`make dev PEPO_LOG resolution: ${scenario.name}`, makeOptions, (t) => {
		const directory = mkdtempSync(path.join(tmpdir(), 'pepo-dev-env-'));
		t.after(() => rmSync(directory, { recursive: true, force: true }));
		const fixture = path.join(directory, 'capture.mjs');
		writeFileSync(
			fixture,
			'console.log(JSON.stringify({ level: process.env.PEPO_LOG, args: process.argv.slice(2) }));\n'
		);
		const quote = (value) => `"${value.replaceAll('\\', '/')}"`;
		const wrapper = `${quote(process.execPath)} ${quote(fixture)}`;
		const environment = { ...process.env, MAKEFLAGS: '', MFLAGS: '' };
		delete environment.PEPO_LOG;
		if (scenario.inherited !== undefined) environment.PEPO_LOG = scenario.inherited;
		const result = spawnSync(
			'make',
			[
				'--silent',
				'--no-print-directory',
				'dev',
				`TOKE=${wrapper}`,
				'PNPM=pnpm',
				...(scenario.override === undefined ? [] : [`PEPO_LOG=${scenario.override}`])
			],
			{ cwd: root, env: environment, encoding: 'utf8' }
		);
		assert.ifError(result.error);
		assert.equal(result.status, 0, result.stdout + result.stderr);
		assert.deepEqual(JSON.parse(result.stdout.trim()), {
			level: scenario.expected,
			args: ['-v', 'pnpm', 'tauri', 'dev']
		});
	});
}
