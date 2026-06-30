<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import { checkForAppUpdates } from '$utils/updater';
	import { Toaster } from '$lib/components/ui/sonner/index.ts';
	import { applyThemePreference } from '$lib/settings';
	import { ErrorToast } from '$utils/inbound';
	import { appSettings, getNormalizedAppSettings } from '$lib/stores/settings.svelte';

	let { children } = $props();
	let normalizedAppSettings = $derived(getNormalizedAppSettings());

	$effect(() => {
		if (!appSettings.ready || typeof document === 'undefined') return;

		return applyThemePreference(normalizedAppSettings.appearance.theme);
	});

	onMount(async () => {
		await checkForAppUpdates('https://github.com/synthlabs/pepo/releases/latest');
	});
</script>

<Toaster theme={normalizedAppSettings.appearance.theme} position="bottom-right" />
<ErrorToast />

<div class="h-full w-full">
	{@render children?.()}
</div>
