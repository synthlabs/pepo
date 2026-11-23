<script lang="ts">
	import Layout from '../../../../../src/routes/app/+layout.svelte';
	import SettingsRoute from '../../../../../src/routes/app/settings/+page.svelte';
	import HomeRoute from '../../../../../src/routes/app/+page.svelte';
	import { page } from './native.svelte';
	import { applyThemePreference } from '$lib/settings';
	import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';

	const settings = $derived(getNormalizedAppSettings());
	$effect(() => applyThemePreference(settings.appearance.theme));
</script>

<Layout>
	{#if page.url.pathname === '/app/settings'}
		<SettingsRoute />
	{:else if page.url.pathname === '/app'}
		<HomeRoute />
	{:else}
		<div class="p-4" data-chat-fixture>Chat navigation fixture</div>
	{/if}
</Layout>
