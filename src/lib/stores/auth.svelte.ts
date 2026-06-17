import { SyncedState } from 'tauri-svelte-synced-store';
import type { AuthState } from '$lib/bindings';

export const authState = new SyncedState<AuthState>('auth_state');