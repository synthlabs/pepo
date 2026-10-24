import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { parse } from 'yaml';
import { collectArtifacts } from './ci-artifacts.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const workflow = (name) => parse(readFileSync(path.join(root, '.github/workflows', name), 'utf8'));
const validation = workflow('validation.yml');

function evaluate(expression, context) {
	const source = expression
		.trim()
		.replace(/^\$\{\{\s*|\s*\}\}$/g, '')
		.replace(/\.([a-zA-Z_][\w-]*)/g, (_, key) => `[${JSON.stringify(key)}]`);
	return vm.runInNewContext(source, context, { timeout: 100 });
}

function fixture(t) {
	const directory = mkdtempSync(path.join(tmpdir(), 'pepo-ci-test-'));
	t.after(() => {
		assert.equal(path.dirname(path.resolve(directory)), path.resolve(tmpdir()));
		rmSync(directory, { recursive: true, force: true });
	});
	return directory;
}

test('pull requests share validation without packaging or secrets', () => {
	const pullRequest = workflow('test.yml');
	assert.deepEqual(Object.keys(pullRequest.on), ['pull_request']);
	assert.equal(pullRequest.permissions.contents, 'read');
	assert.equal(pullRequest.jobs.validation.uses, './.github/workflows/validation.yml');
	assert.equal(pullRequest.jobs.validation.secrets, undefined);
	assert.equal(pullRequest.jobs.validation.with, undefined);
	for (const input of ['packages', 'publish', 'aur-access']) {
		assert.equal(validation.on.workflow_call.inputs[input].default, false);
	}
});

test('branch builds validate packages and AUR access without requesting publication', () => {
	const branch = workflow('test-on-push.yml');
	assert.deepEqual(branch.on.push['branches-ignore'], ['main']);
	assert.equal(branch.permissions.contents, 'read');
	assert.equal(branch.jobs.validation.uses, './.github/workflows/validation.yml');
	assert.deepEqual(branch.jobs.validation.with, { packages: true, 'aur-access': true });
	assert.equal(
		branch.jobs.validation.secrets.TAURI_PEPO_SIGNING_KEY,
		'${{ secrets.TAURI_PEPO_SIGNING_KEY }}'
	);
	const main = workflow('publish-release.yml');
	assert.deepEqual(main.on.push.branches, ['main']);
	assert.equal(main.jobs.validation.uses, './.github/workflows/validation.yml');
	assert.equal(main.jobs.validation.with.publish, true);
});

test('publication requires the input, push event, and main ref together', () => {
	const buildAction = validation.jobs.build.steps.find((step) =>
		step.uses?.startsWith('tauri-apps/')
	);
	const publicationSteps = [
		validation.jobs.arch.steps.find((step) => step.name === 'Upload Arch package to release'),
		validation.jobs.aur.steps.find((step) => step.name === 'Publish pepo-bin')
	];
	for (const publish of [false, true]) {
		for (const eventName of ['push', 'pull_request', 'workflow_dispatch']) {
			for (const ref of ['refs/heads/main', 'refs/heads/fix/ci', 'refs/pull/42/merge']) {
				const permitted = publish && eventName === 'push' && ref === 'refs/heads/main';
				const enabled = evaluate(validation.env.PUBLISH_RELEASE, {
					inputs: { publish },
					github: { event_name: eventName, ref }
				});
				assert.equal(enabled, permitted);
				const context = {
					env: { PUBLISH_RELEASE: String(enabled) },
					steps: { metadata: { outputs: { 'release-tag': 'app-1.0.0-test' } } }
				};
				assert.equal(
					evaluate(buildAction.with.tagName, context),
					permitted ? 'app-1.0.0-test' : ''
				);
				assert.equal(evaluate(buildAction.with.includeUpdaterJson, context), permitted);
				for (const step of publicationSteps) assert.equal(evaluate(step.if, context), permitted);
			}
		}
	}
});

test('native tests and packages cover all platforms without fail-fast', () => {
	assert.deepEqual(
		new Set(validation.jobs.verify.strategy.matrix.platform),
		new Set(['ubuntu-24.04', 'windows-latest', 'macos-latest'])
	);
	assert.equal(validation.jobs.verify.strategy['fail-fast'], false);
	assert.equal(validation.jobs.build.strategy['fail-fast'], false);
	assert.ok(validation.jobs.verify.steps.some((step) => step.run === 'pnpm test:run'));
	assert.ok(
		validation.jobs.verify.steps.some((step) => /cargo test.*--locked/.test(step.run ?? ''))
	);
	const builds = validation.jobs.build.strategy.matrix.include;
	assert.equal(builds.length, 3);
	assert.ok(builds.every((build) => build.args.endsWith('-- --locked')));
	assert.match(
		builds.find((build) => build.platform === 'macos-latest').args,
		/--target universal-apple-darwin/
	);
});

test('release version matches package metadata and the committed Cargo lock', () => {
	const appVersion = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')).version;
	const configVersion = JSON.parse(
		readFileSync(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8')
	).version;
	const manifest = readFileSync(path.join(root, 'src-tauri/Cargo.toml'), 'utf8')
		.split('[package]')[1]
		.split(/\r?\n\[/)[0];
	const manifestVersion = manifest.match(/^version = "([^"]+)"/m)?.[1];
	const lockVersion = readFileSync(path.join(root, 'Cargo.lock'), 'utf8').match(
		/\[\[package\]\]\s+name = "pepo"\s+version = "([^"]+)"/
	)?.[1];
	assert.equal(configVersion, appVersion);
	assert.equal(manifestVersion, appVersion);
	assert.equal(lockVersion, appVersion);
});

test('release metadata keeps the asset tag, filename, and body link consistent', (t) => {
	const directory = fixture(t);
	const output = path.join(directory, 'outputs');
	const result = spawnSync(process.execPath, [path.join(root, 'scripts/ci-metadata.mjs')], {
		cwd: root,
		encoding: 'utf8',
		env: { ...process.env, GITHUB_OUTPUT: output, GITHUB_RUN_NUMBER: '123' }
	});
	assert.equal(result.status, 0, result.stderr);
	const metadata = readFileSync(output, 'utf8');
	const version = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')).version;
	const tag = metadata.match(/^release-tag=(.+)$/m)?.[1];
	assert.ok(tag?.startsWith(`app-${version}-`));
	assert.ok(metadata.includes(`filename=pepo-${version}-123-x86_64.pkg.tar.zst\n`));
	assert.ok(metadata.includes(`https://github.com/synthlabs/pepo/releases/tag/${tag}\n`));
});

for (const [platform, names] of Object.entries({
	Linux: ['sample.deb', 'sample.rpm', 'sample.AppImage', 'sample.AppImage.sig'],
	Windows: ['sample setup.exe', 'sample setup.exe.sig'],
	macOS: ['sample.dmg', 'sample.app.tar.gz', 'sample.app.tar.gz.sig']
})) {
	test(`${platform} artifacts require packages and a matching updater signature`, (t) => {
		const directory = fixture(t);
		for (const name of names.slice(0, -1)) writeFileSync(path.join(directory, name), 'package');
		assert.throws(() => collectArtifacts(directory, platform), /Missing paired updater/);
		writeFileSync(path.join(directory, names.at(-1)), 'signature');
		assert.equal(collectArtifacts(directory, platform).length, names.length);
	});
}

test('partial Linux bundles and unrelated signatures fail validation', (t) => {
	const directory = fixture(t);
	writeFileSync(path.join(directory, 'sample.deb'), 'package');
	writeFileSync(path.join(directory, 'other.sig'), 'signature');
	assert.throws(() => collectArtifacts(directory, 'Linux'), /Missing .rpm package/);
});
