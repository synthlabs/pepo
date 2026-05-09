<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import { checkForAppUpdates } from '$utils/updater';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import type { Settings } from '$lib/bindings';
	import { applyThemePreference, DEFAULT_SETTINGS, normalizeSettings } from '$lib/settings';

	let { children } = $props();
	let settings = new SyncedState<Settings>('settings', DEFAULT_SETTINGS);
	let normalizedSettings = $derived(normalizeSettings(settings.obj));

	$effect(() => {
		if (!settings.ready || typeof document === 'undefined') return;

		return applyThemePreference(normalizedSettings.appearance.theme);
	});

	onMount(async () => {
		await checkForAppUpdates('https://github.com/synthlabs/pepo/releases/latest');
	});
</script>

<div class="h-full w-full">
	{@render children?.()}
</div>
