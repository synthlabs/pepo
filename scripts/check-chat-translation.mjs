import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
function run(command, args) {
	const result = spawnSync(command, args, {
		cwd: root,
		stdio: 'inherit',
		shell: process.platform === 'win32' && command === 'pnpm'
	});
	if (result.error) throw result.error;
	if (result.status !== 0) process.exit(result.status ?? 1);
}

run('cargo', ['test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', 'chat::translation']);
run('pnpm', [
	'exec',
	'vitest',
	'run',
	'src/lib/settings.test.ts',
	'src/lib/settings/controls.test.ts',
	'src/lib/settings/autosave.test.ts',
	'src/lib/chat/translation.test.ts',
	'src/lib/chat/message-history.test.ts',
	'src/lib/components/chat/translation.test.ts',
	'src/lib/components/settings/settings-page.test.ts'
]);
if (process.argv.includes('--browser')) {
	const browser = [
		process.env.BROWSER_BIN,
		'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
		'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
		'/usr/bin/chromium',
		'/usr/bin/chromium-browser',
		'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
	].find((candidate) => candidate && existsSync(candidate));
	if (!browser) throw new Error('Set BROWSER_BIN to an installed Chromium browser');
	run('node', ['scripts/check-settings.mjs']);
	run('node', ['scripts/check-chat-engine.mjs']);
	run('node', ['scripts/check-chat-scroll-route.mjs', '--browser', browser]);
}
