import assert from 'node:assert/strict';
import test from 'node:test';
import path from 'node:path';
import {
	CONTROL_CHECKS,
	LAUNCHES,
	STYLE_CHECKS,
	buildChecklist,
	createResult,
	exitCodeForResult,
	isolatedEnvironment,
	parseArgs,
	validateEnvironment
} from './check-window-controls.mjs';

function passingAnswers() {
	return Object.fromEntries(
		buildChecklist().map((launch) => [
			launch.id,
			Object.fromEntries(launch.checks.map((check) => [check.id, true]))
		])
	);
}

function passingStyleAnswers() {
	return Object.fromEntries(STYLE_CHECKS.map((check) => [check.id, true]));
}

test('builds the approved three-launch control matrix', () => {
	assert.deepEqual(
		LAUNCHES.map(({ id, label }) => ({ id, label })),
		[
			{ id: 'fresh', label: 'Fresh state' },
			{ id: 'restored', label: 'Restored state' },
			{ id: 'maximized', label: 'Saved maximized state' }
		]
	);
	assert.equal(
		buildChecklist().every((launch) => launch.checks.length === CONTROL_CHECKS.length),
		true
	);
});

test('requires a phase and explicit binary', () => {
	assert.throws(() => parseArgs([]), /Missing required --phase/);
	assert.throws(() => parseArgs(['--phase', 'during', '--binary', 'pepo']), /before or after/);
	assert.throws(() => parseArgs(['--phase', 'after']), /Missing required --binary/);
	assert.deepEqual(
		parseArgs([
			'--',
			'--phase',
			'before',
			'--binary',
			'target/release/pepo',
			'--output',
			'result.json',
			'--keep-state'
		]),
		{
			phase: 'before',
			binary: 'target/release/pepo',
			output: 'result.json',
			keepState: true
		}
	);
});

test('accepts KDE Wayland and rejects other interactive baselines', () => {
	assert.doesNotThrow(() =>
		validateEnvironment({ XDG_SESSION_TYPE: 'wayland', XDG_CURRENT_DESKTOP: 'KDE' })
	);
	assert.doesNotThrow(() =>
		validateEnvironment({ XDG_SESSION_TYPE: 'wayland', DESKTOP_SESSION: 'plasma' })
	);
	assert.throws(
		() => validateEnvironment({ XDG_SESSION_TYPE: 'x11', XDG_CURRENT_DESKTOP: 'KDE' }),
		/XDG_SESSION_TYPE=wayland/
	);
	assert.throws(
		() => validateEnvironment({ XDG_SESSION_TYPE: 'wayland', XDG_CURRENT_DESKTOP: 'GNOME' }),
		/KDE Plasma/
	);
});

test('isolates every XDG state directory under one temporary root', () => {
	const environment = isolatedEnvironment({ PATH: '/usr/bin' }, '/tmp/pepo-window-test');

	assert.equal(environment.PATH, '/usr/bin');
	for (const key of ['XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME']) {
		assert.equal(path.dirname(environment[key]), '/tmp/pepo-window-test');
	}
});

test('serializes a passing result with optional thickness metadata', () => {
	const styleAnswers = passingStyleAnswers();
	styleAnswers.thinnerThanBaseline = false;
	const result = createResult({
		phase: 'after',
		answers: passingAnswers(),
		styleAnswers,
		recordedAt: '2026-08-01T12:00:00.000Z'
	});

	assert.equal(result.schemaVersion, 1);
	assert.equal(result.recordedAt, '2026-08-01T12:00:00.000Z');
	assert.equal(result.launches.length, LAUNCHES.length);
	assert.equal(result.style.thinnerThanBaseline, false);
	assert.equal(result.complete, true);
	assert.equal(result.passed, true);
	assert.equal(exitCodeForResult(result), 0);
});

test('only failed after-phase results return a failure exit code', () => {
	const answers = passingAnswers();
	answers.restored.maximize = false;
	const before = createResult({
		phase: 'before',
		answers,
		styleAnswers: passingStyleAnswers()
	});
	const after = createResult({
		phase: 'after',
		answers,
		styleAnswers: passingStyleAnswers()
	});

	assert.equal(before.passed, false);
	assert.equal(exitCodeForResult(before), 0);
	assert.equal(after.passed, false);
	assert.equal(exitCodeForResult(after), 1);
});

test('required dark styling fails while optional thickness does not', () => {
	const darkFailure = passingStyleAnswers();
	darkFailure.darkAndLegible = false;
	const thicknessFailure = passingStyleAnswers();
	thicknessFailure.thinnerThanBaseline = false;

	assert.equal(
		createResult({ phase: 'after', answers: passingAnswers(), styleAnswers: darkFailure }).passed,
		false
	);
	assert.equal(
		createResult({ phase: 'after', answers: passingAnswers(), styleAnswers: thicknessFailure })
			.passed,
		true
	);
});
