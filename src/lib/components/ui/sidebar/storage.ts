import { SIDEBAR_STORAGE_KEY } from './constants';

export function readSidebarOpen(): boolean {
	try {
		return window.localStorage.getItem(SIDEBAR_STORAGE_KEY) !== 'false';
	} catch {
		return true;
	}
}

export function writeSidebarOpen(open: boolean): void {
	try {
		window.localStorage.setItem(SIDEBAR_STORAGE_KEY, String(open));
	} catch {
		// Keep the sidebar usable when client storage is unavailable.
	}
}
