<script lang="ts">
	/* eslint-disable @typescript-eslint/no-explicit-any */
	import { onMount, tick } from 'svelte';
	import Layout from '../../../../../src/routes/app/+layout.svelte';
	import NetworkStatus from '$lib/components/network-status.svelte';
	import SettingsPage from '$lib/components/settings/settings-page.svelte';
	import { applyThemePreference } from '$lib/settings';
	import { appErrors } from '$lib/errors/controller';
	import { native, page, goto, params } from './native.svelte';
	import { settingsState } from './settings.svelte';

	let verdict = $state('running');
	let details = $state('');
	let mounted = $state(true);
	let login = $state(false);
	$effect(() => applyThemePreference(settingsState.value.appearance.theme));

	const pause = (ms = 35) => new Promise((resolve) => setTimeout(resolve, ms));
	function check(condition: unknown, message: string): asserts condition {
		if (!condition) throw new Error(message);
	}
	function button(text: string): HTMLElement {
		const target = [...document.querySelectorAll<HTMLElement>('button,[role="menuitem"]')].find(
			(node) => node.textContent?.trim() === text
		);
		check(target, 'Missing action: ' + text);
		return target;
	}
	async function click(text: string) {
		button(text).click();
		await pause();
	}
	let revision = 0;
	function emit(issues: any[]) {
		native.snapshot = { revision: ++revision, issues };
		native.listeners.get('network_status')?.({ payload: native.snapshot });
	}
	const avatar = () => document.querySelector<HTMLElement>('[data-account-menu-trigger]')!;
	const drawer = () => document.querySelector<HTMLElement>('[data-network-drawer]');
	const tooltip = () =>
		[...document.querySelectorAll<HTMLElement>('[data-network-tooltip]')].find(
			(node) => node.closest('[data-tooltip-content]')?.getAttribute('data-state') !== 'closed'
		);
	function rgb(css: string) {
		const canvas = document.createElement('canvas');
		const context = canvas.getContext('2d')!;
		context.fillStyle = css;
		context.fillRect(0, 0, 1, 1);
		return [...context.getImageData(0, 0, 1, 1).data];
	}
	function backdrop(node: HTMLElement | null) {
		for (; node; node = node.parentElement) {
			const color = rgb(getComputedStyle(node).backgroundColor);
			if (color[3] === 255) return color;
		}
		return rgb('white');
	}
	function contrast(left: number[], right: number[]) {
		const luminance = (color: number[]) =>
			color
				.slice(0, 3)
				.map((value) => value / 255)
				.map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4))
				.reduce((total, value, i) => total + value * [0.2126, 0.7152, 0.0722][i], 0);
		const values = [luminance(left), luminance(right)].sort((a, b) => b - a);
		return (values[0] + 0.05) / (values[1] + 0.05);
	}
	async function closeDrawer() {
		await click('Close');
		await pause(350);
	}

	onMount(() => {
		void (async () => {
			for (let i = 0; !native.initial && i < 40; i++) await pause();
			check(native.initial, 'Native status did not connect');
			check(native.networkListeners === 1, 'More than one native subscription');
			check(
				!document.querySelector('[data-network-avatar-status]'),
				'Healthy avatar showed an indicator'
			);
			const issue = {
				id: 'FFZ emotes:timeout',
				feature: 'FFZ emotes',
				category: 'timeout',
				message: 'A slow request timed out.',
				failures: 17,
				affected: 17,
				cached: true,
				last_failure_at: Date.now(),
				retry_at: Date.now() + 15000,
				retryable: true,
				core: false,
				can_retry: true
			};
			const seven = { ...issue, id: '7tv:timeout', feature: '7TV emotes' };
			const bttv = { ...issue, id: 'bttv:timeout', feature: 'BTTV emotes' };
			for (let i = 1; i <= 17; i++) emit([{ ...issue, failures: i, affected: i }]);
			native.initial({ revision: 0, issues: [] });
			await pause();
			check(
				document.querySelectorAll('[data-network-avatar-status]').length === 1,
				'Startup burst did not produce one avatar indicator'
			);
			const dot = document.querySelector<HTMLElement>('[data-network-avatar-status]')!;
			check(
				contrast(rgb(getComputedStyle(dot).backgroundColor), backdrop(dot.parentElement)) >= 3,
				'Avatar indicator has insufficient contrast'
			);
			check(!document.querySelector('header [data-network-status]'), 'Old header button remained');
			check(
				!document.querySelector('[data-network-header-status]'),
				'Chat route showed a Settings icon'
			);
			check(
				appErrors.snapshot().notifications.length === 0,
				'A transient failure produced a popup'
			);
			emit([seven, bttv, { ...seven, id: '7tv:connection' }]);
			await pause();
			avatar().focus();
			await pause(250);
			check(tooltip(), 'Avatar focus did not show the tooltip');
			check(
				tooltip()!.textContent?.includes('Some features are still refreshing in the background:'),
				'Tooltip heading changed'
			);
			check(
				[...tooltip()!.querySelectorAll('li')].map((node) => node.textContent).join('|') ===
					'7TV emotes|BTTV emotes',
				'Feature list was duplicated or out of order'
			);
			avatar().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
			await pause(200);
			check(!tooltip(), 'Escape did not dismiss the tooltip');
			avatar().blur();
			avatar().dispatchEvent(new PointerEvent('pointerenter', { pointerType: 'mouse' }));
			await pause(250);
			check(tooltip(), 'Avatar hover did not show the tooltip');
			avatar().click();
			await pause(200);
			check(document.querySelector('[role="menu"]'), 'Account avatar stopped opening its menu');
			check(
				avatar().getAttribute('data-state') === 'open',
				'Account menu lost its open-state styling'
			);
			check(!tooltip() && !drawer(), 'Account menu opened status UI');
			await click('Settings');
			await pause(250);
			check(page.url.pathname === '/app/settings', 'Settings navigation failed');
			check(
				!document.querySelector('[data-network-avatar-status]'),
				'Settings retained the avatar indicator'
			);
			const icon = document.querySelector<HTMLElement>('[data-network-header-status]')!;
			check(
				icon && document.querySelector('header h1')?.parentElement?.contains(icon),
				'Settings status icon was outside the left title group'
			);
			check(
				contrast(rgb(getComputedStyle(icon.querySelector('svg')!).color), backdrop(icon)) >= 3,
				'Settings status icon has insufficient contrast'
			);
			icon.focus();
			await pause(250);
			check(tooltip(), 'Settings focus did not show tooltip');
			icon.click();
			await pause();
			check(!drawer(), 'Settings status icon opened the drawer');
			emit([
				bttv,
				seven,
				{ ...seven, id: '7tv:unavailable', retryable: false },
				{ ...issue, id: 'chat', feature: 'Chat', core: true }
			]);
			button('Background activity').focus();
			await pause();
			icon.focus();
			await pause(250);
			check(
				tooltip()!.textContent?.includes('Reconnecting:') &&
					tooltip()!.textContent?.includes('Some features need attention:'),
				'Urgent states were labeled as refreshing'
			);
			check(
				[...tooltip()!.querySelectorAll('li')].filter((node) => node.textContent === '7TV emotes')
					.length === 1,
				'Mixed severity duplicated a feature'
			);
			check(icon.querySelector('.text-destructive'), 'Urgent status had no distinct icon');
			emit([issue]);
			await pause();
			await click('Background activity');
			await pause(250);
			check(drawer(), 'Chat settings did not open the drawer');
			check(
				document.querySelectorAll('[data-network-issues] > li').length === 1,
				'Failure group was duplicated'
			);
			check(
				drawer()!.textContent?.includes('Using saved data'),
				'Cache fallback was not explained'
			);
			await click('Retry now');
			check(native.retries[0] === issue.id, 'Retry targeted the wrong incident');
			await click('Report problem');
			const frozen = appErrors.snapshot().report;
			check(frozen?.error?.message.includes('17'), 'Report lost the selected failure counts');
			emit([]);
			await pause();
			check(
				!document.querySelector('[data-network-header-status]'),
				'Recovered icon remained visible'
			);
			check(appErrors.snapshot().report?.id === frozen?.id, 'Recovery changed the selected report');
			appErrors.closeReport();
			await pause(350);
			await click('Background activity');
			check(
				drawer()!.textContent?.includes('Everything has caught up.'),
				'Healthy drawer button did not work'
			);
			await closeDrawer();
			check(
				document.activeElement === button('Background activity'),
				'Drawer did not restore focus'
			);
			await click('App');
			await click('Advanced');
			const input = document.querySelector<HTMLInputElement>(
				'[id="setting-providers-http_request_timeout_secs"]'
			);
			check(input?.value === '15', 'Saved provider timeout was overwritten');
			input.value = '45';
			input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new FocusEvent('blur', { bubbles: true }));
			await pause();
			check(
				settingsState.value.providers.http_request_timeout_secs === 45,
				'Advanced timeout did not save'
			);
			check(
				settingsState.value.network.request_timeout_secs === 30,
				'Provider override changed the API timeout'
			);
			await goto('/app/chat/maya');
			emit([seven, bttv]);
			await pause();
			emit([bttv]);
			await pause();
			check(
				document.querySelector('[data-network-avatar-status]'),
				'Partial recovery hid remaining issues'
			);
			check(
				native.networkListeners === 1 && native.snapshots === 1,
				'Navigation duplicated native status work'
			);
			mounted = false;
			await tick();
			await pause();
			check(native.networkListeners === 0, 'Unmount leaked the status subscription');
			login = true;
			await tick();
			await pause();
			const loginButton = document.querySelector<HTMLElement>('[data-network-status]');
			check(loginButton, 'Standalone login status button disappeared');
			loginButton.click();
			await pause();
			check(drawer(), 'Standalone login button did not open the drawer');
			await closeDrawer();
			check(document.activeElement === loginButton, 'Standalone drawer lost its focus return');
			login = false;
			await tick();
			await pause();
			check(native.networkListeners === 0, 'Login status leaked its subscription');
			mounted = true;
			await tick();
			await pause();
			check(
				native.networkListeners === 1 && native.snapshots === 3,
				'Remount did not establish exactly one subscription'
			);
			emit([seven, bttv]);
			await pause();
			const view = params.get('view') ?? 'chat';
			if (view !== 'chat') {
				await goto('/app/settings');
				await pause();
			}
			if (view === 'drawer') {
				await click('Background activity');
				await pause(600);
			} else {
				(view === 'chat'
					? avatar()
					: document.querySelector<HTMLElement>('[data-network-header-status]')!
				).focus();
				await pause(250);
				check(tooltip(), 'Final tooltip was unavailable');
			}
			check(
				document.documentElement.scrollWidth <= window.innerWidth,
				'Status UI overflowed the desktop width'
			);
			verdict = 'passed';
			details =
				'Avatar/menu, settings tooltip, shared drawer, recovery, frozen report, subscription cleanup and network settings passed.';
		})().catch((error: unknown) => {
			verdict = 'failed';
			details = error instanceof Error ? (error.stack ?? error.message) : String(error);
		});
	});
</script>

{#if mounted}
	<Layout>
		{#if page.url.pathname === '/app/settings'}
			<SettingsPage />
		{:else}
			<p class="p-4 text-sm">Chat remains available while emotes refresh.</p>
		{/if}
	</Layout>
{/if}
{#if login}
	<NetworkStatus />
{/if}
<output data-network-check={verdict} class="sr-only">{details}</output>
