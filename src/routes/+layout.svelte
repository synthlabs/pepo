<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { PUBLIC_UPDATE_INSTALL_MODE } from '$env/static/public';
	import '../app.css';
	import { checkForAppUpdates, type UpdateAction } from '$utils/updater';
	import { Toaster } from '$lib/components/ui/sonner/index.ts';
	import { applyThemePreference } from '$lib/settings';
	import ErrorHost from '$lib/components/error-host.svelte';
	import { networkHealth } from '$lib/network/health';
	import { withBackgroundRequest } from '$lib/network/request';
	import { errorDetails } from '$utils/inbound';
	import { appSettings, getNormalizedAppSettings } from '$lib/stores/settings.svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import isTauri from '$lib/tauri';
	import { appErrors } from '$lib/errors/controller';

	let { children } = $props();
	let normalizedAppSettings = $derived(getNormalizedAppSettings());

	const updateAction: UpdateAction =
		PUBLIC_UPDATE_INSTALL_MODE === 'package-manager'
			? {
					kind: 'external',
					label: 'Update via package manager',
					url: 'https://aur.archlinux.org/packages/pepo-bin'
				}
			: { kind: 'install' };

	$effect(() => {
		if (!appSettings.ready || typeof document === 'undefined') return;
		if (isTauri) {
			const current = getCurrentWindow();
			if (current.label.startsWith('user-card-')) {
				const theme = normalizedAppSettings.appearance.theme;
				void current.setTheme(theme === 'system' ? null : theme).catch((error) =>
					appErrors.capture(error, {
						source: 'user-card.theme',
						title: 'Could not update window theme'
					})
				);
			}
		}

		return applyThemePreference(normalizedAppSettings.appearance.theme);
	});

	onMount(() => {
		if (isTauri && getCurrentWindow().label !== 'main') return;
		const abort = new AbortController();
		void (async () => {
			try {
				await appSettings.initialized;
			} catch {
				// Already captured by appSettings singleton; fall back to defaults.
			}
			await tick();
			if (abort.signal.aborted) return;
			await checkForAppUpdates('https://github.com/synthlabs/pepo/releases/latest', {
				updateAction,
				withRequest: withBackgroundRequest,
				signal: abort.signal,
				requestTimeoutMs: getNormalizedAppSettings().network.request_timeout_secs * 1000,
				onStatus: ({ error, retryInMs }) =>
					networkHealth.setExternal(
						'external:updates',
						'Update checks',
						error === null ? null : errorDetails(error),
						false,
						{
							retryAt: retryInMs === undefined ? undefined : Date.now() + retryInMs,
							newAttempt: true
						}
					)
			});
		})();
		return () => abort.abort();
	});
</script>

<Toaster theme={normalizedAppSettings.appearance.theme} position="bottom-right" />
<ErrorHost />

<div class="h-full w-full">
	{@render children?.()}
</div>
