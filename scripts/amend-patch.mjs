#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function amendPatch(run = spawnSync) {
	function git(args, options = {}) {
		const result = run('git', ['--literal-pathspecs', ...args], {
			encoding: 'utf8',
			...options
		});
		if (result.error) {
			throw result.error;
		}
		if (result.status !== 0) {
			const detail = result.stderr?.trim();
			throw new Error(
				`git ${args[0]} failed (${result.status ?? result.signal ?? 'unknown'})${detail ? `: ${detail}` : ''}`
			);
		}
		return result.stdout;
	}

	const committed = git([
		'diff-tree',
		'--root',
		'--no-commit-id',
		'--name-only',
		'--no-renames',
		'--diff-filter=d',
		'-r',
		'-z',
		'HEAD'
	])
		.split('\0')
		.filter(Boolean);
	const changed = new Set(
		git(['diff', '--name-only', '--no-renames', '--ignore-submodules=dirty', '-z', 'HEAD', '--'])
			.split('\0')
			.filter(Boolean)
	);
	const selected = committed.filter((file) => changed.has(file));
	if (selected.length === 0) {
		return false;
	}

	git(
		['commit', '--amend', '--no-edit', '--only', '--pathspec-from-file=-', '--pathspec-file-nul'],
		{
			input: `${selected.join('\0')}\0`,
			stdio: ['pipe', 'inherit', 'inherit']
		}
	);
	return true;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try {
		amendPatch();
	} catch (error) {
		console.error(error.message);
		process.exitCode = 1;
	}
}
