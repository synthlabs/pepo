<script lang="ts">
	import subBadge from '$resources/sub.svelte';
	import moderatorBadge from '$resources/moderator.svelte';
	import partnerBadge from '$resources/partner.svelte';
	import broadcasterBadge from '$resources/broadcaster.svelte';
	import vipBadge from '$resources/vip.svelte';

	export let badge: [string, string];

	const badgeMap: Map<string, any> = new Map<string, any>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);
</script>

<svelte:component this={badgeMap.get(badge[0])} />
