import { describe, expect, it } from 'vitest';
import { UserDirectory } from './users.svelte';
import { formatUsername, usernameColor } from './username';

const maya = { user_id: '1', login: 'maya', display_name: '마야', color: '#123456' };

describe('session user directory', () => {
	it('resolves case-insensitive logins, replaces snapshots, and respects known IDs', () => {
		const users = new UserDirectory();
		users.begin('session');
		users.apply({ type: 'snapshot', users: [maya] });
		expect(users.get(null, 'MAYA')).toEqual(maya);
		expect(users.get('unknown', 'maya')).toBeUndefined();
		users.apply({ type: 'delta', upserts: [{ ...maya, login: 'renamed' }], removed: [] });
		expect(users.get(null, 'maya')).toBeUndefined();
		expect(users.get('1', 'maya')?.login).toBe('renamed');
		users.apply({ type: 'snapshot', users: [] });
		expect(users.get('1', 'renamed')).toBeUndefined();
	});
	it('removes evictions and clears metadata on session replacement and logout', () => {
		const users = new UserDirectory();
		users.begin('one');
		users.apply({ type: 'snapshot', users: [maya] });
		users.begin('one');
		expect(users.get(null, 'maya')).toEqual(maya);
		users.apply({ type: 'delta', upserts: [], removed: ['1'] });
		expect(users.get(null, 'maya')).toBeUndefined();
		users.apply({ type: 'snapshot', users: [maya] });
		users.begin('two');
		expect(users.get('1', 'maya')).toBeUndefined();
		users.apply({ type: 'snapshot', users: [maya] });
		users.clear();
		expect(users.get(null, 'maya')).toBeUndefined();
	});
});

describe('username formatting', () => {
	it.each([
		['Maya', 'maya', true, 'Maya'],
		['마야', 'maya', true, '마야 (maya)'],
		['마야', 'maya', false, '마야'],
		['', 'maya', true, 'maya'],
		[null, 'maya', false, 'maya']
	])('formats %s / %s with showLogin=%s', (name, login, showLogin, expected) => {
		expect(formatUsername(name, login, showLogin)).toBe(expected);
	});
	it('accepts only Twitch hex colors and otherwise inherits surrounding text color', () => {
		expect(usernameColor('#Ab12Ef')).toBe('#Ab12Ef');
		for (const color of ['', null, 'red', '#123', 'url(example)', '#nothex'])
			expect(usernameColor(color)).toBeUndefined();
	});
});
