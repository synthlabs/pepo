<script lang="ts">
	import subBadge from '$lib/resources/sub.svelte';
	import moderatorBadge from '$lib/resources/moderator.svelte';
	import partnerBadge from '$lib/resources/partner.svelte';
	import broadcasterBadge from '$lib/resources/broadcaster.svelte';
	import vipBadge from '$lib/resources/vip.svelte';
	import type { BadgeRef } from '$lib/bindings';
	import type { Component } from 'svelte';

	interface Props {
		badge_ref: BadgeRef;
		sizePx?: number;
	}

	let { badge_ref, sizePx = 20 }: Props = $props();

	let fallback = $derived(
		badge_ref.set_id !== badge_ref.badge.set_id && badge_ref.id !== badge_ref.badge.id
	);

	const badgeMap: Map<string, Component> = new Map<string, Component>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);
</script>

{#if fallback}
	<!-- svelte-ignore svelte_component_deprecated -->
	<svelte:component this={badgeMap.get(badge_ref.set_id)} {sizePx} />
{:else}
	<img
		class="inline max-w-none"
		style="height: {sizePx}px;"
		src={badge_ref.badge.image_url_4x}
		alt={badge_ref.badge.description}
	/>
{/if}
