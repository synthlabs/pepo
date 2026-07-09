#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const FORMAT_PATTERNS = [
	'src/**/*.{svelte,ts,js,css,json,md}',
	'utils/js/**/*.{svelte,ts,js,json,md}',
	'AGENTS.md',
	'README.md',
	'components.json',
	'eslint.config.js',
	'package.json',
	'postcss.config.js',
	'svelte.config.js',
	'tailwind.config.ts',
	'tsconfig.json',
	'vite.config.js',
	'vitest.config.ts',
	'.prettierrc',
	'.vscode/**/*.json'
];

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

export function runPrettier(mode) {
	if (mode !== '--check' && mode !== '--write') {
		console.error('usage: node scripts/format.mjs --check|--write');
		return 2;
	}

	const result = spawnCommand(localBin('prettier'), [mode, ...FORMAT_PATTERNS]);

	if (result.error) {
		console.error(result.error.message);
		return 1;
	}

	return result.status ?? 1;
}

const thisFilePath = fileURLToPath(import.meta.url);

if (process.argv[1] && path.resolve(process.argv[1]) === thisFilePath) {
	process.exitCode = runPrettier(process.argv[2]);
}
