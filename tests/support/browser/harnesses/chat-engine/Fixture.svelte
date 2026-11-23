<script lang="ts">
	import Layout from '../../../../../src/routes/app/+layout.svelte';
	import Chat from '../../../../../src/routes/app/chat/[id]/+page.svelte';
	import Settings from '../../../../../src/routes/app/settings/+page.svelte';
	import { page } from './native.svelte';
	import { applyThemePreference } from '$lib/settings';
	import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';

	const settings = $derived(getNormalizedAppSettings());
	$effect(() => applyThemePreference(settings.appearance.theme));
</script>

<Layout>
	{#if page.params.id}
		<Chat />
	{:else}
		<Settings />
	{/if}
</Layout>
