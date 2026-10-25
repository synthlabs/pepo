import { SyncedState } from 'tauri-svelte-synced-store';
import { untrack } from 'svelte';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS, normalizeAppSettings } from '$lib/settings';
import {
	SettingsAutosave,
	type SettingsUpdate,
	type SettingsSnapshot
} from '$lib/settings/autosave';

export const appSettings = new SyncedState<AppSettings>(
	'app_settings',
	structuredClone(DEFAULT_APP_SETTINGS)
);
export const settingsState = $state<SettingsSnapshot>({
	value: structuredClone(DEFAULT_APP_SETTINGS),
	ready: false,
	saving: false,
	result: 'idle'
});
let outgoing: AppSettings | undefined;
const autosave = new SettingsAutosave(
	async (settings) => {
		appSettings.obj = settings;
		outgoing = appSettings.obj;
		return appSettings.sync();
	},
	(snapshot) => Object.assign(settingsState, snapshot)
);

$effect.root(() => {
	$effect(() => {
		const incoming = appSettings.obj;
		if (!appSettings.ready || incoming === outgoing) return;
		untrack(() => autosave.receive(normalizeAppSettings($state.snapshot(incoming))));
	});
});

export function updateAppSettings(update: SettingsUpdate): Promise<boolean> {
	return autosave.update(typeof update === 'function' ? update : $state.snapshot(update));
}

export function getNormalizedAppSettings(): AppSettings {
	return settingsState.value;
}
