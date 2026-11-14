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
import TranslationConfirmation from './translation-confirmation.svelte';
import { getNormalizedAppSettings, updateAppSettings } from '$lib/stores/settings.svelte';
import { TranslationSession } from '$lib/translation/session.svelte';
import { EN_RU, fixture, local, settle } from '$lib/translation/test-fixtures';

let component: ReturnType<typeof mount> | undefined;
let status: ReturnType<typeof mount> | undefined;
let confirmation: ReturnType<typeof mount> | undefined;
const sessions: TranslationSession[] = [];
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
async function translationPage() {
	await unmount(component!);
	const f = fixture();
	const session = new TranslationSession(f.client);
	sessions.push(session);
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(SettingsPage, { target, props: { translationSession: session } });
	confirmation = mount(TranslationConfirmation, { target, props: { session } });
	await flushUi();
	await click('[aria-label="Settings categories"] button:nth-child(3)');
	return { ...f, session };
}
async function chooseLanguage(value: string) {
	const select = element<HTMLSelectElement>('#setting-translation-target_language');
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
	await flushUi();
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
	if (confirmation) await unmount(confirmation);
	for (const session of sessions.splice(0)) await session.dispose();
	await flushUi();
	component = undefined;
	status = undefined;
	confirmation = undefined;
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.useRealTimers();
});

describe('settings page', () => {
	it('exposes the notice toggle in advanced Chat settings, saves opt-out, and finds it by search', async () => {
		expect(document.querySelector('#setting-chat-moderation_notices_enabled')).toBeNull();
		await click('[aria-controls="settings-more-chat"]');
		expect(element('#setting-chat-moderation_notices_enabled').getAttribute('aria-checked')).toBe(
			'true'
		);
		expect(document.body.textContent).toContain(
			'Add ban, timeout, and chat-clear notices to open chats.'
		);
		await click('#setting-chat-moderation_notices_enabled');
		expect(getNormalizedAppSettings().chat.moderation_notices_enabled).toBe(false);
		await click('[aria-controls="settings-more-chat"]');
		await search('moderation notices');
		expect(element('#setting-chat-moderation_notices_enabled').getAttribute('aria-checked')).toBe(
			'false'
		);
		expect(native.writes.at(-1)?.chat.moderation_notices_enabled).toBe(false);
	});
	it('shows three translation preferences and asks consent before downloading or enabling', async () => {
		const f = await translationPage();
		expect(
			Array.from(document.querySelectorAll('[data-setting-row]'), (row) =>
				row.getAttribute('data-setting-row')
			)
		).toEqual(['translation.enabled', 'translation.target_language', 'translation.layout']);
		expect(document.querySelector('[data-model-manager]')).toBeNull();
		expect(f.client.watchState).not.toHaveBeenCalled();
		await click('#setting-translation-enabled');
		expect(element('[role="alertdialog"]').textContent).toContain('Download translation support?');
		expect(element('[role="alertdialog"]').textContent).toContain('Download & enable');
		expect(element('[role="alertdialog"]').textContent?.replace(/\s+/g, ' ')).toContain(
			'Pepo needs a small language model to translate messages into English.'
		);
		expect(f.client.startInstall).not.toHaveBeenCalled();
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		f.session.respond(false);
		await flushUi();
		expect(element('#setting-translation-enabled').getAttribute('aria-checked')).toBe('false');
		expect(f.client.startInstall).not.toHaveBeenCalled();
		await click('#setting-translation-enabled');
		f.session.respond(true);
		await flushUi();
		expect(document.querySelector('[data-translation-setup]')).not.toBeNull();
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		await click('[aria-label="Settings categories"] button:first-child');
		f.finish();
		await flushUi();
		expect(getNormalizedAppSettings().translation.enabled).toBe(true);
		await click('[aria-label="Settings categories"] button:nth-child(3)');
		expect(element('#setting-translation-enabled').getAttribute('aria-checked')).toBe('true');
	});
	it('keeps the previous target on declined consent and saves a new target only after setup succeeds', async () => {
		const f = await translationPage();
		f.rows([local()]);
		await click('#setting-translation-enabled');
		expect(getNormalizedAppSettings().translation.enabled).toBe(true);
		await chooseLanguage('rus');
		expect(f.session.confirmation?.pair).toEqual(EN_RU);
		expect(element('[role="alertdialog"]').textContent?.replace(/\s+/g, ' ')).toContain(
			'Pepo needs a small language model to translate messages into Russian.'
		);
		expect(getNormalizedAppSettings().translation.target_language).toBe('eng');
		f.session.respond(false);
		await flushUi();
		expect(element<HTMLSelectElement>('#setting-translation-target_language').value).toBe('eng');
		await chooseLanguage('rus');
		f.session.respond(true);
		await flushUi();
		expect(getNormalizedAppSettings().translation.target_language).toBe('eng');
		f.finish();
		await flushUi();
		expect(getNormalizedAppSettings().translation.target_language).toBe('rus');
	});
	it.each([
		{ intent: 'enable', searching: false },
		{ intent: 'enable', searching: true },
		{ intent: 'language', searching: false },
		{ intent: 'language', searching: true }
	] as const)(
		'keeps quick installed-model checks quiet for $intent (search: $searching)',
		async ({ intent, searching }) => {
			const f = await translationPage();
			f.rows([local(), local(EN_RU)]);
			if (intent === 'language') await click('#setting-translation-enabled');
			if (searching) await search(intent === 'enable' ? 'translate chat' : 'translate into');
			for (let attempt = 0; attempt < 2; attempt++) {
				let finish!: (value: ReturnType<typeof f.state>) => void;
				f.client.status.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
				try {
					const previous = { ...getNormalizedAppSettings().translation };
					const nextLanguage = previous.target_language === 'eng' ? 'rus' : 'eng';
					if (intent === 'enable') element('#setting-translation-enabled').click();
					else {
						const select = element<HTMLSelectElement>('#setting-translation-target_language');
						select.value = nextLanguage;
						select.dispatchEvent(new Event('change', { bubbles: true }));
					}
					await settle();
					flushSync();
					const preparing = () =>
						Array.from(document.querySelectorAll('[role="status"]')).find(
							(node) => node.textContent?.trim() === 'Preparing translation…'
						);
					expect(preparing()).toBeUndefined();
					expect(getNormalizedAppSettings().translation).toEqual(previous);
					await vi.advanceTimersByTimeAsync(299);
					flushSync();
					expect(preparing()).toBeUndefined();
					finish(f.state());
					await flushUi();
					expect(getNormalizedAppSettings().translation).toEqual({
						...previous,
						...(intent === 'enable' ? { enabled: true } : { target_language: nextLanguage })
					});
					await vi.advanceTimersByTimeAsync(301);
					flushSync();
					expect(preparing()).toBeUndefined();
					if (intent === 'enable') await click('#setting-translation-enabled');
				} finally {
					finish(f.state());
					await flushUi();
				}
			}
			expect(f.client.planInstall).not.toHaveBeenCalled();
			expect(f.client.startInstall).not.toHaveBeenCalled();
		}
	);
	it('shows preparation after 300 ms without restarting on model updates and hides it on completion', async () => {
		const f = await translationPage();
		f.rows([local()]);
		await f.session.connect();
		await flushUi();
		let finish!: (value: ReturnType<typeof f.state>) => void;
		f.client.status.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
		try {
			element('#setting-translation-enabled').click();
			await settle();
			flushSync();
			await vi.advanceTimersByTimeAsync(200);
			f.emit({});
			flushSync();
			await vi.advanceTimersByTimeAsync(99);
			flushSync();
			expect(document.body.textContent).not.toContain('Preparing translation…');
			await vi.advanceTimersByTimeAsync(1);
			flushSync();
			expect(document.body.textContent).toContain('Preparing translation…');
			finish(f.state());
			await flushUi();
			expect(document.body.textContent).not.toContain('Preparing translation…');
			expect(getNormalizedAppSettings().translation.enabled).toBe(true);
		} finally {
			finish(f.state());
			await flushUi();
		}
	});
	it('clears a pending preparation timer when navigating away and starts a fresh delay on return', async () => {
		const f = await translationPage();
		f.rows([local()]);
		await f.session.connect();
		await flushUi();
		let finish!: (value: ReturnType<typeof f.state>) => void;
		f.client.status.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
		try {
			await click('#setting-translation-enabled');
			await vi.advanceTimersByTimeAsync(200);
			await click('[aria-label="Settings categories"] button:first-child');
			await vi.advanceTimersByTimeAsync(300);
			flushSync();
			expect(document.body.textContent).not.toContain('Preparing translation…');
			element('[aria-label="Settings categories"] button:nth-child(3)').click();
			flushSync();
			await vi.advanceTimersByTimeAsync(299);
			flushSync();
			expect(document.body.textContent).not.toContain('Preparing translation…');
			await vi.advanceTimersByTimeAsync(1);
			flushSync();
			expect(document.body.textContent).toContain('Preparing translation…');
			finish(f.state());
			await flushUi();
			expect(document.body.textContent).not.toContain('Preparing translation…');
		} finally {
			finish(f.state());
			await flushUi();
		}
	});
	it('cancels setup without enabling translation and selects a language without downloading while off', async () => {
		const f = await translationPage();
		await chooseLanguage('rus');
		expect(f.client.watchState).not.toHaveBeenCalled();
		expect(getNormalizedAppSettings().translation.target_language).toBe('rus');
		await click('#setting-translation-enabled');
		f.session.respond(true);
		await flushUi();
		await click('[data-translation-cancel]');
		expect(f.client.cancelInstall).toHaveBeenCalledTimes(1);
		f.finish({ code: 'cancelled', message: 'Cancelled' });
		await flushUi();
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		expect(document.querySelector('[data-translation-setup-error]')).toBeNull();
	});
	it('applies the same download consent when enabling from search', async () => {
		const f = await translationPage();
		await search('translate chat');
		await click('#setting-translation-enabled');
		expect(f.session.confirmation?.action).toBe('install');
		expect(f.client.startInstall).not.toHaveBeenCalled();
		f.session.respond(false);
		await flushUi();
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
	});
	it('shows setup failures and prevents a late download completion from undoing a reset', async () => {
		const f = await translationPage();
		f.emit({ phase: 'unavailable' });
		await click('#setting-translation-enabled');
		expect(element('[data-translation-setup-error]').textContent).toContain(
			'Try again or open Advanced'
		);
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		f.emit({ phase: 'ready' });
		await click('#setting-translation-enabled');
		f.session.respond(true);
		await flushUi();
		await click('[data-open-translation-models]');
		await click('[aria-label="Reset settings"] > button');
		[...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')]
			.find((button) => button.textContent === 'Reset to defaults')!
			.click();
		await flushUi();
		f.finish();
		await flushUi();
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		expect(f.client.cancelInstall).not.toHaveBeenCalled();
		expect(f.client.removeModel).not.toHaveBeenCalled();
	});
	it('finds model management and saves translation preferences with independent reset and rollback', async () => {
		await unmount(component!);
		const f = fixture();
		f.rows([local(), local(EN_RU)]);
		const session = new TranslationSession(f.client);
		const target = document.createElement('div');
		document.body.append(target);
		component = mount(SettingsPage, { target, props: { translationSession: session } });
		await flushUi();
		await search('models');
		await click('[data-open-translation-models]');
		await settle();
		await flushUi();
		expect(document.activeElement?.id).toBe('translation-models');
		expect(document.querySelector('#setting-translation-layout')).toBeNull();
		expect(
			Array.from(
				document.querySelectorAll('[data-setting-row]'),
				(row) => (row as HTMLElement).dataset.settingRow
			)
		).toEqual([
			'translation.translate_history',
			'translation.detection_policy',
			'translation.queue_policy',
			'translation.queue_capacity',
			'translation.max_in_flight'
		]);
		expect(f.client.watchState).toHaveBeenCalledTimes(1);
		expect(getNormalizedAppSettings().translation.enabled).toBe(false);
		await click('[data-translation-back]');
		expect(document.activeElement?.hasAttribute('data-open-translation-models')).toBe(true);
		const layout = element<HTMLSelectElement>('#setting-translation-layout');
		layout.value = 'connector';
		layout.dispatchEvent(new Event('change', { bubbles: true }));
		await flushUi();
		await click('#setting-translation-enabled');
		const select = element<HTMLSelectElement>('#setting-translation-target_language');
		select.value = 'rus';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushUi();
		expect(getNormalizedAppSettings().translation).toEqual({
			...DEFAULT_APP_SETTINGS.translation,
			enabled: true,
			target_language: 'rus',
			layout: 'connector'
		});
		native.save = async () => false;
		await click('#setting-translation-enabled');
		expect(getNormalizedAppSettings().translation.enabled).toBe(true);
		native.save = async (value) => {
			native.emit(value);
			return true;
		};
		await updateAppSettings({ chat: { message_limit: 900 } });
		await flushUi();
		await click('[data-open-translation-models]');
		await click('[aria-label="Reset settings"] > button');
		[...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')]
			.find((button) => button.textContent === 'Reset to defaults')!
			.click();
		await flushUi();
		expect(getNormalizedAppSettings().translation).toEqual(DEFAULT_APP_SETTINGS.translation);
		expect(getNormalizedAppSettings().chat.message_limit).toBe(900);
		expect(f.client.removeModel).not.toHaveBeenCalled();
		expect(f.client.cancelInstall).not.toHaveBeenCalled();
		await unmount(component!);
		component = undefined;
		await session.dispose();
	});
	it('starts with Chat essentials and searches editable collapsed controls and choice labels', async () => {
		expect(document.querySelectorAll('[data-setting-row]')).toHaveLength(6);
		expect(document.querySelector('[data-setting-row="translation.layout"]')).toBeNull();
		await search('connector');
		const select = element<HTMLSelectElement>('#setting-translation-layout');
		select.value = 'connector';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await flushUi();
		expect(getNormalizedAppSettings().translation.layout).toBe('connector');
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
