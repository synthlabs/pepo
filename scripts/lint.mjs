#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

function localBin(name) {
	const suffix = process.platform === 'win32' ? '.CMD' : '';
	const candidate = path.join(process.cwd(), 'node_modules', '.bin', `${name}${suffix}`);

	return existsSync(candidate) ? candidate : name;
}

function quoteCmdArg(value) {
	if (!/[ \t&()^|<>"]/.test(value)) {
		return value;
	}

	return `"${value.replaceAll('"', '""')}"`;
}

function spawnCommand(command, args) {
	if (process.platform === 'win32' && command.toLowerCase().endsWith('.cmd')) {
		return spawnSync(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', [command, ...args].map(quoteCmdArg).join(' ')], {
			stdio: 'inherit'
		});
	}

	return spawnSync(command, args, { stdio: 'inherit' });
}

function runStep(label, command, args) {
	console.log(`\n> ${label}`);

	const result = spawnCommand(command, args);

	if (result.error) {
		console.error(result.error.message);
		return 1;
	}

	return result.status ?? 1;
}

const steps = [
	['SvelteKit sync', localBin('svelte-kit'), ['sync']],
	['svelte-check', localBin('svelte-check'), ['--tsconfig', './tsconfig.json']],
	['ESLint', localBin('eslint'), ['.']],
	['Tailwind class conflicts', process.execPath, ['scripts/lint-tailwind-classes.mjs']],
	['Prettier format check', process.execPath, ['scripts/format.mjs', '--check']]
];

let failed = false;

for (const [label, command, args] of steps) {
	const status = runStep(label, command, args);
	if (status !== 0) {
		failed = true;
	}
}

process.exitCode = failed ? 1 : 0;
