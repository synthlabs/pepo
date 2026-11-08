import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const compile = process.argv.includes('--compile');
const nativeOnly = process.argv.includes('--native');
function run(command, args, cwd = root) {
	const result = spawnSync(command, args, {
		cwd,
		stdio: 'inherit',
		env: { ...process.env, CARGO_TARGET_DIR: `${root}/target` },
		shell: process.platform === 'win32' && command === 'pnpm'
	});
	if (result.error) throw result.error;
	if (result.status !== 0) process.exit(result.status ?? 1);
}
run('cargo', ['test', '--offline', '--lib', ...(compile ? ['--no-run'] : [])], `${root}/src-tauri`);
if (!compile) {
	run('cargo', ['test', '--offline', '--manifest-path', 'utils/rust/inbound/Cargo.toml']);
	run('cargo', [
		'test',
		'--offline',
		'--manifest-path',
		'utils/rust/translate/Cargo.toml',
		'--features',
		'tauri-test'
	]);
	if (!nativeOnly) {
		run('pnpm', ['test:run']);
		run('node', ['scripts/check-network-ui.mjs']);
		run('node', ['scripts/check-chat-submit.mjs']);
	}
}
