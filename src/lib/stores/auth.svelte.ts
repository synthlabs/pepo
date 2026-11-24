import { SyncedState } from 'tauri-svelte-synced-store';
import type { AuthState } from '$lib/bindings';
import { appErrors } from '$lib/errors/controller';

export const DEFAULT_AUTH_STATE: AuthState = {
	phase: 'unauthorized',
	device_code: '',
	token: null
};

export const authState = new SyncedState<AuthState>(
	'auth_state',
	structuredClone(DEFAULT_AUTH_STATE)
);

void authState.initialized.catch((error) => {
	appErrors.capture(error, {
		source: 'synced-store.auth_state',
		title: 'Could not sync authentication state'
	});
});

import.meta.hot?.dispose(() => {
	void authState.close();
});
