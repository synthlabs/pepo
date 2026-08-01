import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import { access, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { createInterface } from 'node:readline/promises';
import { pathToFileURL } from 'node:url';

export const LAUNCHES = [
	{
		id: 'fresh',
		label: 'Fresh state',
		start: 'The window should open restored with no saved window state.',
		finish: 'Leave the window restored before closing it.'
	},
	{
		id: 'restored',
		label: 'Restored state',
		start: 'The window should restore the normal state saved by the first launch.',
		finish: 'Maximize the window again and leave it maximized before closing it.'
	},
	{
		id: 'maximized',
		label: 'Saved maximized state',
		start: 'The window should open maximized from the state saved by the second launch.',
		finish: 'Restore the window before closing it.'
	}
];

export const CONTROL_CHECKS = [
	{
		id: 'minimize',
		prompt: 'Without double-clicking the title bar, did Minimize respond to one click?'
	},
	{
		id: 'maximize',
		prompt: 'Did Maximize respond to one click?'
	},
	{
		id: 'restore',
		prompt: 'Did Restore respond to one click?'
	},
	{
		id: 'close',
		prompt: 'After following the finish instruction, did Close respond to one click?'
	}
];

export const STYLE_CHECKS = [
	{
		id: 'darkAndLegible',
		prompt: 'Is the native title bar dark with legible title text and controls?',
		required: true
	},
	{
		id: 'thinnerThanBaseline',
		prompt: 'Is the native title bar thinner than the 48px reported baseline?',
		required: false
	}
];

const USAGE =
	'Usage: pnpm check:window-controls -- --phase <before|after> --binary <path> [--output <path>] [--keep-state]';

export function parseArgs(args) {
	const options = { keepState: false };

	for (let index = 0; index < args.length; index += 1) {
		const argument = args[index];
		if (argument === '--') continue;
		if (argument === '--help' || argument === '-h') return { help: true };
		if (argument === '--keep-state') {
			options.keepState = true;
			continue;
		}
		if (!['--phase', '--binary', '--output'].includes(argument)) {
			throw new Error(`Unknown argument: ${argument}`);
		}

		const value = args[index + 1];
		if (!value || value.startsWith('--')) throw new Error(`Missing value for ${argument}`);
		options[argument.slice(2)] = value;
		index += 1;
	}

	if (!options.phase) throw new Error('Missing required --phase option');
	if (!['before', 'after'].includes(options.phase)) {
		throw new Error('--phase must be either before or after');
	}
	if (!options.binary) throw new Error('Missing required --binary option');

	return options;
}

export function validateEnvironment(environment) {
	if (environment.XDG_SESSION_TYPE?.toLowerCase() !== 'wayland') {
		throw new Error('This acceptance check requires XDG_SESSION_TYPE=wayland');
	}

	const desktop = [environment.XDG_CURRENT_DESKTOP, environment.DESKTOP_SESSION]
		.filter(Boolean)
		.join(':')
		.toLowerCase();
	if (!desktop.includes('kde') && !desktop.includes('plasma')) {
		throw new Error('This acceptance check requires a KDE Plasma session');
	}
}

export function buildChecklist() {
	return LAUNCHES.map((launch) => ({
		...launch,
		checks: CONTROL_CHECKS.map((check) => ({ ...check }))
	}));
}

export function createResult({
	phase,
	answers,
	styleAnswers,
	recordedAt = new Date().toISOString()
}) {
	const launches = buildChecklist().map((launch) => {
		const checks = Object.fromEntries(
			launch.checks.map((check) => {
				const answer = answers[launch.id]?.[check.id];
				return [check.id, typeof answer === 'boolean' ? answer : null];
			})
		);
		const complete = Object.values(checks).every((answer) => typeof answer === 'boolean');

		return {
			id: launch.id,
			label: launch.label,
			checks,
			complete,
			passed: complete && Object.values(checks).every(Boolean)
		};
	});
	const style = Object.fromEntries(
		STYLE_CHECKS.map((check) => {
			const answer = styleAnswers[check.id];
			return [check.id, typeof answer === 'boolean' ? answer : null];
		})
	);
	const requiredStylePassed = STYLE_CHECKS.filter((check) => check.required).every(
		(check) => style[check.id] === true
	);
	const complete =
		launches.every((launch) => launch.complete) &&
		Object.values(style).every((answer) => typeof answer === 'boolean');

	return {
		schemaVersion: 1,
		phase,
		platform: 'CachyOS KDE Wayland',
		recordedAt,
		launches,
		style,
		complete,
		passed: complete && launches.every((launch) => launch.passed) && requiredStylePassed
	};
}

export function exitCodeForResult(result) {
	return result.phase === 'after' && !result.passed ? 1 : 0;
}

export function isolatedEnvironment(baseEnvironment, stateRoot) {
	return {
		...baseEnvironment,
		XDG_CACHE_HOME: path.join(stateRoot, 'cache'),
		XDG_CONFIG_HOME: path.join(stateRoot, 'config'),
		XDG_DATA_HOME: path.join(stateRoot, 'data'),
		XDG_STATE_HOME: path.join(stateRoot, 'state')
	};
}

async function askBoolean(readline, prompt) {
	while (true) {
		const answer = (await readline.question(`${prompt} [y/n] `)).trim().toLowerCase();
		if (answer === 'y' || answer === 'yes') return true;
		if (answer === 'n' || answer === 'no') return false;
		process.stdout.write('Please answer y or n.\n');
	}
}

function waitForExit(child, timeoutMs = 5000) {
	if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();

	return new Promise((resolve, reject) => {
		const timeout = setTimeout(() => {
			reject(new Error('Pepo did not exit after the Close check'));
		}, timeoutMs);
		child.once('exit', () => {
			clearTimeout(timeout);
			resolve();
		});
	});
}

async function runLaunch(binary, environment, launch, readline, styleAnswers) {
	process.stdout.write(`\n${launch.label}: ${launch.start}\n`);
	const child = spawn(binary, [], {
		env: environment,
		stdio: ['ignore', 'inherit', 'inherit']
	});
	const spawnError = new Promise((_, reject) => child.once('error', reject));
	const answers = {};

	try {
		for (const check of launch.checks) {
			if (check.id === 'close') {
				if (launch.id === 'fresh') {
					process.stdout.write('\nNative title-bar presentation:\n');
					for (const styleCheck of STYLE_CHECKS) {
						styleAnswers[styleCheck.id] = await Promise.race([
							askBoolean(readline, styleCheck.prompt),
							spawnError
						]);
					}
				}
				process.stdout.write(`${launch.finish}\n`);
			}
			answers[check.id] = await Promise.race([askBoolean(readline, check.prompt), spawnError]);
		}

		if (!answers.close && child.exitCode === null) child.kill('SIGTERM');
		await waitForExit(child);
	} finally {
		if (child.exitCode === null) child.kill('SIGTERM');
	}

	return answers;
}

async function ensureStateDirectories(environment) {
	await Promise.all(
		['XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'].map((key) =>
			mkdir(environment[key], { recursive: true })
		)
	);
}

async function run() {
	let options;
	try {
		options = parseArgs(process.argv.slice(2));
		if (!options.help) validateEnvironment(process.env);
	} catch (error) {
		process.stderr.write(`${error.message}\n${USAGE}\n`);
		process.exitCode = 2;
		return;
	}

	if (options.help) {
		process.stdout.write(`${USAGE}\n`);
		return;
	}

	const binary = path.resolve(options.binary);
	try {
		await access(binary, constants.X_OK);
	} catch {
		process.stderr.write(`Pepo binary is not executable: ${binary}\n`);
		process.exitCode = 2;
		return;
	}

	const stateRoot = await mkdtemp(path.join(tmpdir(), 'pepo-window-controls-'));
	const environment = isolatedEnvironment(process.env, stateRoot);
	await ensureStateDirectories(environment);
	const readline = createInterface({ input: process.stdin, output: process.stdout });
	const answers = {};
	const styleAnswers = {};

	try {
		process.stdout.write(
			`Recording ${options.phase} window-control results with isolated state at ${stateRoot}.\n`
		);
		for (const launch of buildChecklist()) {
			answers[launch.id] = await runLaunch(binary, environment, launch, readline, styleAnswers);
		}
	} finally {
		readline.close();
	}

	const result = createResult({ phase: options.phase, answers, styleAnswers });
	const serialized = `${JSON.stringify(result, null, 2)}\n`;

	if (options.output) {
		await writeFile(options.output, serialized, 'utf8');
		process.stdout.write(`\nWrote ${options.output}\n`);
	} else {
		process.stdout.write(`\n${serialized}`);
	}

	if (options.keepState) {
		process.stdout.write(`Kept isolated state at ${stateRoot}\n`);
	} else {
		await rm(stateRoot, { recursive: true, force: true });
	}

	process.exitCode = exitCodeForResult(result);
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : null;
if (invokedUrl === import.meta.url) {
	await run();
}
