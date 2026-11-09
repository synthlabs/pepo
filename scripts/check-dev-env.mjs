import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const fixture = path.join(root, 'target/dev-env-check/capture.mjs');
mkdirSync(path.dirname(fixture), { recursive: true });
writeFileSync(
	fixture,
	'console.log(JSON.stringify({ level: process.env.PEPO_LOG, args: process.argv.slice(2) }));\n'
);
const quote = (value) => `"${value.replaceAll('\\', '/')}"`;
const wrapper = `${quote(process.execPath)} ${quote(fixture)}`;

for (const scenario of [
	{ name: 'default', inherited: undefined, override: undefined, expected: 'debug' },
	{ name: 'inherited preference', inherited: 'info', override: undefined, expected: 'info' },
	{ name: 'explicit override', inherited: 'info', override: 'trace', expected: 'trace' }
]) {
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
	if (result.error) throw result.error;
	assert.equal(result.status, 0, result.stdout + result.stderr);
	assert.deepEqual(
		JSON.parse(result.stdout.trim()),
		{
			level: scenario.expected,
			args: ['-v', 'pnpm', 'tauri', 'dev']
		},
		scenario.name
	);
	console.log(`PASS make dev logging: ${scenario.name}`);
}
