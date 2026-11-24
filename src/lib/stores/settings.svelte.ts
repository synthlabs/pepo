import { SyncedState } from 'tauri-svelte-synced-store';
import { untrack } from 'svelte';
import type { AppSettings } from '$lib/bindings';
import { appErrors } from '$lib/errors/controller';
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
void appSettings.initialized.catch((error) => {
	appErrors.capture(error, {
		source: 'synced-store.app_settings',
		title: 'Could not sync app settings'
	});
});
export const settingsState = $state<SettingsSnapshot>({
	value: structuredClone(DEFAULT_APP_SETTINGS),
	ready: false,
	saving: false,
	result: 'idle'
});
let outgoing: AppSettings | undefined;
const autosave = new SettingsAutosave(
	async (settings) => {
		const previous = appSettings.obj;
		appSettings.obj = settings;
		outgoing = appSettings.obj;
		try {
			const saved = await appSettings.sync();
			if (!saved && appSettings.obj === outgoing) {
				appSettings.obj = previous;
				outgoing = appSettings.obj;
			}
			return saved;
		} catch (error) {
			if (appSettings.obj === outgoing) {
				appSettings.obj = previous;
				outgoing = appSettings.obj;
			}
			throw error;
		}
	},
	(snapshot) => Object.assign(settingsState, snapshot)
);

const stop = $effect.root(() => {
	$effect(() => {
		const incoming = appSettings.obj;
		if (!appSettings.ready || incoming === outgoing) return;
		untrack(() => autosave.receive(normalizeAppSettings($state.snapshot(incoming))));
	});
});

import.meta.hot?.dispose(() => {
	stop();
	void appSettings.close();
});

export function updateAppSettings(update: SettingsUpdate): Promise<boolean> {
	return autosave.update(typeof update === 'function' ? update : $state.snapshot(update));
}

export function getNormalizedAppSettings(): AppSettings {
	return settingsState.value;
}
