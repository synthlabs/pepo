import { DEFAULT_APP_SETTINGS } from '$lib/settings';
import { applySettingsUpdate } from '$lib/settings/autosave';
import type { AppSettings } from '$lib/bindings';

const value = structuredClone(DEFAULT_APP_SETTINGS);
const params = new URLSearchParams(location.search);
localStorage.setItem('sidebar:state', String(params.get('open') !== 'false'));
value.appearance.theme = (params.get('theme') ?? 'dark') as AppSettings['appearance']['theme'];
value.providers.http_connect_timeout_secs = 5;
value.providers.http_request_timeout_secs = 15;

export const settingsState = $state({ value, ready: true, saving: false, result: 'idle' });
export const getNormalizedAppSettings = () => settingsState.value;
export async function updateAppSettings(
	update: Partial<AppSettings> | ((current: AppSettings) => AppSettings)
) {
	settingsState.value = applySettingsUpdate(settingsState.value, update);
	return true;
}
