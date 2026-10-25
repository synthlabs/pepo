import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';

const native = vi.hoisted(() => ({
	listeners: new Map<string, ((event: { payload: AppSettings }) => void)[]>(),
	writes: [] as AppSettings[],
	save: undefined as ((value: AppSettings) => Promise<boolean>) | undefined,
	emit(value: AppSettings) {
		for (const listener of this.listeners.get('app_settings_update') ?? [])
			listener({ payload: structuredClone(value) });
	}
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: async (name: string, listener: (event: { payload: AppSettings }) => void) => {
		const listeners = native.listeners.get(name) ?? [];
		listeners.push(listener);
		native.listeners.set(name, listeners);
		return () => {
			native.listeners.set(
				name,
				listeners.filter((item) => item !== listener)
			);
		};
	}
}));
vi.mock('@tauri-apps/api/core', () => ({
	invoke: async (command: string, args: { state?: { value: string } }) => {
		if (command === 'emit_state') return true;
		if (command !== 'update_state' || !args.state) throw new Error(`Unexpected command ${command}`);
		const value = JSON.parse(args.state.value) as AppSettings;
		native.writes.push(value);
		return native.save!(value);
	}
}));

import SettingsPage from './settings-page.svelte';
import SettingsStatus from './settings-status.svelte';
import { getNormalizedAppSettings, updateAppSettings } from '$lib/stores/settings.svelte';

let component: ReturnType<typeof mount> | undefined;
let status: ReturnType<typeof mount> | undefined;
let sequence = 0;

async function flushUi() {
	for (let pass = 0; pass < 3; pass++) {
		flushSync();
		await tick();
		await vi.advanceTimersByTimeAsync(1);
	}
}
function element<T extends HTMLElement = HTMLElement>(selector: string) {
	const found = document.querySelector<T>(selector);
	expect(found, selector).not.toBeNull();
	return found!;
}
async function click(selector: string) {
	element(selector).click();
	await flushUi();
}
async function search(value: string) {
	const input = element<HTMLInputElement>('[aria-label="Search settings"]');
	input.value = value;
	input.dispatchEvent(new Event('input', { bubbles: true }));
	await flushUi();
}
async function edit(selector: string, value: string, key = 'Enter') {
	const input = element<HTMLInputElement>(selector);
	input.value = value;
	input.dispatchEvent(new Event('input', { bubbles: true }));
	input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
	await flushUi();
	return input;
}

beforeEach(async () => {
	vi.useFakeTimers();
	vi.spyOn(console, 'log').mockImplementation(() => {});
	vi.spyOn(console, 'warn').mockImplementation((...args) => {
		throw new Error(args.map(String).join(' '));
	});
	await flushUi();
	native.writes = [];
	native.save = async (value) => {
		native.emit(value);
		return true;
	};
	const baseline = structuredClone(DEFAULT_APP_SETTINGS);
	baseline.channel_cache.poll_interval_secs = 1000 + sequence++;
	native.emit(baseline);
	await flushUi();
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(SettingsPage, { target });
	status = mount(SettingsStatus, { target });
	await flushUi();
});

afterEach(async () => {
	if (component) await unmount(component);
	if (status) await unmount(status);
	await flushUi();
	component = undefined;
	status = undefined;
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.useRealTimers();
});

describe('settings page', () => {
	it('starts with Chat essentials and searches editable collapsed controls and choice labels', async () => {
		expect(document.querySelectorAll('[data-setting-row]')).toHaveLength(4);
		expect(document.querySelector('[data-setting-row="chat.translation_layout"]')).toBeNull();
		await search('connector');
		const select = element<HTMLSelectElement>('#setting-chat-translation_layout');
		select.value = 'connector';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushUi();
		expect(getNormalizedAppSettings().chat.translation_layout).toBe('connector');
		await search('no such setting');
		expect(document.body.textContent).toContain('No settings match');
		await click('[aria-label="Settings categories"] button:last-child');
		expect(element<HTMLInputElement>('[aria-label="Search settings"]').value).toBe('');
		expect(document.querySelectorAll('[data-setting-row]')).toHaveLength(3);
	});

	it('commits numeric input on Enter once and retains invalid edits without a save', async () => {
		await search('message history limit');
		const input = await edit('#setting-chat-message_limit', '900');
		input.dispatchEvent(new FocusEvent('blur'));
		await flushUi();
		expect(native.writes).toHaveLength(1);
		expect(getNormalizedAppSettings().chat.message_limit).toBe(900);
		await edit('#setting-chat-message_limit', '0');
		expect(element('[role="alert"]').textContent).toContain('Enter a whole number');
		expect(native.writes).toHaveLength(1);
		expect(getNormalizedAppSettings().chat.message_limit).toBe(900);
		await search('timestamp locale');
		await edit('#setting-chat-timestamp_locale', 'en_XX');
		expect(element('[role="alert"]').textContent).toContain('valid locale');
		expect(native.writes).toHaveLength(1);
	});

	it('saves valid field input on blur and supports theme choices', async () => {
		await search('timestamp locale');
		const input = element<HTMLInputElement>('#setting-chat-timestamp_locale');
		input.value = 'en-GB';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		await flushUi();
		expect(native.writes).toHaveLength(0);
		input.dispatchEvent(new FocusEvent('blur'));
		await flushUi();
		expect(getNormalizedAppSettings().chat.timestamp_locale).toBe('en-GB');
		await search('app theme');
		const light = Array.from(
			document.querySelectorAll<HTMLButtonElement>('[data-setting-row="appearance.theme"] button')
		).find((button) => button.textContent === 'Light')!;
		light.click();
		await flushUi();
		expect(getNormalizedAppSettings().appearance.theme).toBe('light');
	});

	it('reorders providers without re-enabling disabled entries', async () => {
		await click('[aria-label="Settings categories"] button:nth-child(2)');
		await click('#setting-emotes-providers-twitch');
		await click('[aria-controls="settings-more-emotes"]');
		await click('[aria-label="Move BTTV up"]');
		expect(getNormalizedAppSettings().emotes.providers.slice(0, 2)).toEqual([
			{ id: 'bttv', enabled: true },
			{ id: 'twitch', enabled: false }
		]);
		expect(element<HTMLButtonElement>('[aria-label="Move BTTV up"]').disabled).toBe(true);
	});

	it('confirms and cancels section reset, then resets all visible preferences without touching operational settings', async () => {
		await updateAppSettings({ appearance: { theme: 'light' }, chat: { message_limit: 900 } });
		await flushUi();
		const hidden = getNormalizedAppSettings().channel_cache.poll_interval_secs;
		await click('[aria-label="Reset settings"] > button');
		expect(element('[role="alertdialog"]').textContent).toContain('Reset Chat settings?');
		const cancel = Array.from(
			document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')
		).find((button) => button.textContent === 'Cancel')!;
		cancel.click();
		await flushUi();
		expect(getNormalizedAppSettings().chat.message_limit).toBe(900);
		await click('[aria-label="Reset settings"] > button');
		const confirm = () =>
			Array.from(document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')).find(
				(button) => button.textContent === 'Reset to defaults'
			)!;
		confirm().click();
		await flushUi();
		expect(getNormalizedAppSettings().chat.message_limit).toBe(500);
		expect(getNormalizedAppSettings().appearance.theme).toBe('light');
		await click('[aria-label="More reset actions"]');
		await click('[role="menuitem"]');
		expect(element('[role="alertdialog"]').textContent).toContain('Reset all settings?');
		confirm().click();
		await flushUi();
		expect(getNormalizedAppSettings().appearance.theme).toBe('dark');
		expect(getNormalizedAppSettings().channel_cache.poll_interval_secs).toBe(hidden);
	});

	it('restores failed changes, exposes status tooltips, and clears failure after a later success', async () => {
		native.save = async () => false;
		await click('#setting-chat-show_badges');
		expect(element('[data-settings-status]').getAttribute('aria-label')).toContain('Save failed');
		expect(element('#setting-chat-show_badges').getAttribute('aria-checked')).toBe('true');
		element('[data-settings-status]').focus();
		await flushUi();
		expect(element('[data-settings-status-tooltip]').textContent).toContain(
			'Previous value restored'
		);
		native.save = async (value) => {
			native.emit(value);
			return true;
		};
		await click('#setting-chat-show_badges');
		expect(element('[data-settings-status]').getAttribute('aria-label')).toBe('Settings saved');
	});

	it('finishes a save after the page unmounts and exposes the saved value on return', async () => {
		let finish!: (saved: boolean) => void;
		native.save = () =>
			new Promise<boolean>((resolve) => {
				finish = resolve;
			});
		await click('#setting-chat-show_timestamps');
		expect(element('[data-settings-status]').getAttribute('aria-label')).toContain('Saving');
		await unmount(component!);
		component = undefined;
		finish(true);
		await flushUi();
		const target = document.createElement('div');
		document.body.append(target);
		component = mount(SettingsPage, { target });
		await flushUi();
		expect(element('#setting-chat-show_timestamps').getAttribute('aria-checked')).toBe('false');
		expect(element('[data-settings-status]').getAttribute('aria-label')).toBe('Settings saved');
	});
});
