<script lang="ts">
	import subBadge from '$lib/resources/sub.svelte';
	import moderatorBadge from '$lib/resources/moderator.svelte';
	import partnerBadge from '$lib/resources/partner.svelte';
	import broadcasterBadge from '$lib/resources/broadcaster.svelte';
	import vipBadge from '$lib/resources/vip.svelte';
	import type { BadgeRef } from '$lib/bindings';

	interface Props {
		badge_ref: BadgeRef;
	}

	let { badge_ref }: Props = $props();

	let fallback = $derived(
		badge_ref.set_id !== badge_ref.badge.set_id && badge_ref.id !== badge_ref.badge.id
	);

	$inspect(badge_ref);

	const badgeMap: Map<string, any> = new Map<string, any>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);
</script>

{#if fallback}
	<!-- svelte-ignore svelte_component_deprecated -->
	<svelte:component this={badgeMap.get(badge_ref.set_id)} />
{:else}
	<img
		class="inline h-5 max-w-none"
		src={badge_ref.badge.image_url_4x}
		alt={badge_ref.badge.description}
	/>
{/if}
