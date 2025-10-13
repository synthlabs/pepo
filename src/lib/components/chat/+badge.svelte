<script lang="ts">
	import subBadge from '$lib/resources/sub.svelte';
	import moderatorBadge from '$lib/resources/moderator.svelte';
	import partnerBadge from '$lib/resources/partner.svelte';
	import broadcasterBadge from '$lib/resources/broadcaster.svelte';
	import vipBadge from '$lib/resources/vip.svelte';
	import type { Badge } from '$lib/bindings';

	interface Props {
		badge: Badge;
	}

	let { badge }: Props = $props();

	const badgeMap: Map<string, any> = new Map<string, any>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);
</script>

<!-- svelte-ignore svelte_component_deprecated -->
<svelte:component this={badgeMap.get(badge.set_id)} />
