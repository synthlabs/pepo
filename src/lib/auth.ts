import type { AuthState } from '$lib/bindings';

export function hasUsableAuth(authState: AuthState): boolean {
	return authState.phase === 'authorized' && authState.token !== null;
}
