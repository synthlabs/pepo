import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { amendPatch } from './amend-patch.mjs';

function mockGit(committed, changed, failureAt = -1, failure = {}) {
	const calls = [];
	return {
		calls,
		run(command, args, options) {
			const index = calls.length;
			calls.push({ command, args, options });
			if (index === failureAt) {
				return failure;
			}
			const files = index === 0 ? committed : index === 1 ? changed : [];
			return { status: 0, stdout: files.length ? `${files.join('\0')}\0` : '', stderr: '' };
		}
	};
}

test('amends only changed paths already in HEAD, preserving unrelated staged changes', () => {
	const git = mockGit(
		['package.json', 'SUMRY.md', 'src-tauri/Cargo.toml'],
		['package.json', 'unrelated-staged.ts', 'unrelated-unstaged.ts']
	);
	assert.equal(amendPatch(git.run), true);
	assert.equal(git.calls.length, 3);
	assert.deepEqual(git.calls[2], {
		command: 'git',
		args: [
			'--literal-pathspecs',
			'commit',
			'--amend',
			'--no-edit',
			'--only',
			'--pathspec-from-file=-',
			'--pathspec-file-nul'
		],
		options: {
			encoding: 'utf8',
			input: 'package.json\0',
			stdio: ['pipe', 'inherit', 'inherit']
		}
	});
});

test('skips amendment when HEAD paths have no changes', () => {
	for (const changed of [[], ['unrelated.ts']]) {
		const git = mockGit(['package.json'], changed);
		assert.equal(amendPatch(git.run), false);
		assert.equal(git.calls.length, 2);
	}
});

test('preserves filenames with spaces, brackets, pathspec syntax, Unicode, and newlines', () => {
	const files = [
		'a b.json',
		'[literal].ts',
		':(glob)*.ts',
		'-leading',
		'日本語.ts',
		'line\nbreak.ts'
	];
	const git = mockGit(files, [...files].reverse());
	assert.equal(amendPatch(git.run), true);
	assert.equal(git.calls[2].options.input, `${files.join('\0')}\0`);
	assert.equal(git.calls[2].args[0], '--literal-pathspecs');
});

test('selects rename destinations and excludes paths deleted by the patch commit', () => {
	const git = mockGit(['new name.ts'], ['old name.ts', 'new name.ts', 'deleted.ts']);
	amendPatch(git.run);
	assert.ok(git.calls[0].args.includes('--no-renames'));
	assert.ok(git.calls[0].args.includes('--diff-filter=d'));
	assert.ok(git.calls[1].args.includes('--no-renames'));
	assert.equal(git.calls[2].options.input, 'new name.ts\0');
});

test('includes initial-commit files and ignores dirty submodule contents', () => {
	const git = mockGit(['package.json', 'utils'], ['package.json']);
	amendPatch(git.run);
	assert.ok(git.calls[0].args.includes('--root'));
	assert.ok(git.calls[1].args.includes('--ignore-submodules=dirty'));
	assert.equal(git.calls[2].options.input, 'package.json\0');
});

for (const [index, stage] of ['diff-tree', 'diff', 'commit'].entries()) {
	test(`stops when git ${stage} cannot start`, () => {
		const error = new Error('spawn git ENOENT');
		const git = mockGit(['package.json'], ['package.json'], index, { error });
		assert.throws(
			() => amendPatch(git.run),
			(caught) => caught === error
		);
		assert.equal(git.calls.length, index + 1);
	});

	test(`stops when git ${stage} exits unsuccessfully`, () => {
		const git = mockGit(['package.json'], ['package.json'], index, {
			status: 17,
			stderr: 'example failure\n'
		});
		assert.throws(() => amendPatch(git.run), new RegExp(`git ${stage} failed \\(17\\)`));
		assert.equal(git.calls.length, index + 1);
	});
}

test('reports a Git process terminated by a signal', () => {
	const git = mockGit([], [], 0, { status: null, signal: 'SIGTERM', stderr: '' });
	assert.throws(() => amendPatch(git.run), /SIGTERM/);
});

function findMake() {
	const name = process.platform === 'win32' ? 'make.exe' : 'make';
	return (process.env.PATH ?? '')
		.split(path.delimiter)
		.map((directory) => path.join(directory, name))
		.find((candidate) => {
			if (!existsSync(candidate)) return false;
			const result = spawnSync(candidate, ['--version'], { encoding: 'utf8' });
			return result.status === 0 && /GNU Make/.test(result.stdout);
		});
}

const make = findMake();

function makeFixture(t) {
	const directory = mkdtempSync(path.join(tmpdir(), 'pepo patch (test)-'));
	t.after(() => {
		assert.equal(path.dirname(path.resolve(directory)), path.resolve(tmpdir()));
		rmSync(directory, { recursive: true, force: true });
	});
	mkdirSync(path.join(directory, 'utils/packaging/tauri'), { recursive: true });
	mkdirSync(path.join(directory, 'scripts'));
	writeFileSync(
		path.join(directory, 'Makefile'),
		readFileSync(new URL('../Makefile', import.meta.url))
	);
	writeFileSync(path.join(directory, 'utils/packaging/tauri/tauri.mk'), '');
	const stepSource = `
import { appendFileSync } from 'node:fs';
const step = process.argv[2] ?? 'amend';
appendFileSync('steps.log', step + '\\n');
process.exitCode = process.env.FAIL_STEP === step ? 23 : 0;
`;
	writeFileSync(path.join(directory, 'step.mjs'), stepSource);
	writeFileSync(path.join(directory, 'scripts/amend-patch.mjs'), stepSource);
	let executable = make;
	if (process.platform !== 'win32') {
		// macOS tool launchers dispatch using the executable basename.
		executable = path.join(directory, 'make');
		symlinkSync(make, executable);
	}
	const node = `"${process.execPath.replaceAll('\\', '/')}"`;
	return {
		directory,
		run(args = [], env = {}, { stubPatch = true } = {}) {
			return spawnSync(
				executable,
				[
					'--no-print-directory',
					...args,
					'patch',
					...(process.platform !== 'win32' ? [`MAKE=${executable}`] : []),
					...(stubPatch ? [`PATCH_CMD=${node} step.mjs patch`] : []),
					`TOKE=${node} step.mjs format`
				],
				{
					cwd: directory,
					encoding: 'utf8',
					env: { ...process.env, MAKEFLAGS: '', MFLAGS: '', FAIL_STEP: '', ...env }
				}
			);
		},
		steps() {
			const log = path.join(directory, 'steps.log');
			return existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n') : [];
		}
	};
}

const makeOptions = { skip: make ? false : 'GNU Make is not available' };

test(
	'Makefile runs patch, recursive format, and amend in order with spaced paths',
	makeOptions,
	(t) => {
		const fixture = makeFixture(t);
		const result = fixture.run();
		assert.ifError(result.error);
		assert.equal(result.status, 0, result.stdout + result.stderr);
		assert.deepEqual(fixture.steps(), ['patch', 'format', 'amend']);
	}
);

for (const [index, stage] of ['patch', 'format', 'amend'].entries()) {
	test(`Makefile stops after ${stage} fails`, makeOptions, (t) => {
		const fixture = makeFixture(t);
		const result = fixture.run([], { FAIL_STEP: stage });
		assert.ifError(result.error);
		assert.notEqual(result.status, 0);
		assert.deepEqual(fixture.steps(), ['patch', 'format', 'amend'].slice(0, index + 1));
	});
}

test('make -n patch executes no release steps', makeOptions, (t) => {
	const fixture = makeFixture(t);
	const result = fixture.run(['-n']);
	assert.ifError(result.error);
	assert.deepEqual(fixture.steps(), []);
	if (
		result.status !== 0 &&
		process.platform === 'win32' &&
		/^GNU Make 3\.81\r?$/m.test(spawnSync(make, ['--version'], { encoding: 'utf8' }).stdout) &&
		result.stderr.includes('CreateProcess(NULL, "", ...) failed.') &&
		result.stderr.includes('make (e=87)')
	) {
		t.skip('Installed GNU Make 3.81 fails quoted recursive dry runs with CreateProcess error 87');
		return;
	}
	assert.equal(result.status, 0, result.stdout + result.stderr);
	assert.match(result.stdout, /step\.mjs patch/);
	assert.match(result.stdout, /step\.mjs format/);
	assert.match(result.stdout, /node scripts\/amend-patch\.mjs/);
});

for (const [os, expected] of [
	['Windows_NT', 'patch.ps1'],
	['Linux', 'patch.sh']
]) {
	test(`Makefile selects ${expected} for OS=${os}`, makeOptions, (t) => {
		const fixture = makeFixture(t);
		const result = fixture.run(['-qp', `OS=${os}`], {}, { stubPatch: false });
		assert.ifError(result.error);
		assert.equal(result.status, 1, result.stdout + result.stderr);
		assert.match(result.stdout, new RegExp(`utils/scripts/${expected.replace('.', '\\.')}`));
		assert.deepEqual(fixture.steps(), []);
	});
}
