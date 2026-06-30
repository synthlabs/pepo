import { describe, expect, it } from 'vitest';
import type { AuthState, UserToken } from '$lib/bindings';
import { hasUsableAuth } from './auth';

const token: UserToken = {
	access_token: 'access',
	client_id: 'client',
	login: 'viewer',
	user_id: '1234',
	refresh_token: 'refresh',
	expires_in: 3600,
	profile_image_url: ''
};

function authState(state: Partial<AuthState>): AuthState {
	return {
		phase: 'unauthorized',
		device_code: '',
		token: null,
		...state
	};
}

describe('auth helpers', () => {
	it('requires authorized phase and a token', () => {
		expect(hasUsableAuth(authState({ phase: 'authorized', token }))).toBe(true);
		expect(hasUsableAuth(authState({ phase: 'authorized', token: null }))).toBe(false);
		expect(hasUsableAuth(authState({ phase: 'unauthorized', token }))).toBe(false);
	});

	it('rejects in-progress auth states', () => {
		expect(hasUsableAuth(authState({ phase: 'waitingForDeviceCode' }))).toBe(false);
		expect(hasUsableAuth(authState({ phase: 'waitingForAuth' }))).toBe(false);
		expect(hasUsableAuth(authState({ phase: 'failedAuth' }))).toBe(false);
	});
});
