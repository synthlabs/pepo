import { SyncedState } from 'tauri-svelte-synced-store';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS, normalizeAppSettings } from '$lib/settings';

export const appSettings = new SyncedState<AppSettings>('app_settings', DEFAULT_APP_SETTINGS);

export function getNormalizedAppSettings(): AppSettings {
	return normalizeAppSettings(appSettings.obj);
}
