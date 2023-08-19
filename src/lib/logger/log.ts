import { PUBLIC_DEBUG_LOGS, PUBLIC_TRACE_LOGS } from '$env/static/public';

const debugLogs: boolean = JSON.parse(PUBLIC_DEBUG_LOGS);
const traceLogs: boolean = JSON.parse(PUBLIC_TRACE_LOGS);

export class Logger {
	info(...args: unknown[]) {
		const message = [...generatePrefix('INFO', '#3ABFF8'), ...args];
		console.log(...message);
	}
	warn(...args: unknown[]) {
		const message = [...generatePrefix('WARN', '#FBBD23'), ...args];
		console.log(...message);
	}
	error(...args: unknown[]) {
		const message = [...generatePrefix('ERROR', '#F87272'), ...args];
		console.log(...message);
	}
	debug(...args: unknown[]) {
		if (!debugLogs) return;
		const message = [...generatePrefix('DEBUG', '#D926A9'), ...args];
		console.log(...message);
	}
	trace(...args: unknown[]) {
		if (!traceLogs) return;
		const message = [...generatePrefix('TRACE', '#d95c26'), ...args];
		console.log(...message);
	}
}

export default new Logger();

function generatePrefix(namespace: string, color: string): string[] {
	return [`%c[${namespace}]:%c`, `color:${color}; font-weight:bold`, ''];
}
