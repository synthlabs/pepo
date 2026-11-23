import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../..', import.meta.url));
let command;
let args;

if (process.platform === 'win32') {
	command = 'powershell.exe';
	args = [
		'-NoProfile',
		'-ExecutionPolicy',
		'Bypass',
		'-File',
		'utils/packaging/tauri/install.test.ps1'
	];
} else if (process.platform === 'linux' || process.platform === 'darwin') {
	command = 'bash';
	args = ['tests/packaging/integration.test.sh'];
} else {
	console.error('Mocked packaging tests support Linux, macOS, and Windows.');
	process.exit(1);
}

const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
if (result.error) {
	console.error('Unable to run mocked packaging tests: ' + result.error.message);
}
if (result.signal) {
	console.error('Mocked packaging tests stopped by ' + result.signal);
}
process.exit(result.status ?? 1);
