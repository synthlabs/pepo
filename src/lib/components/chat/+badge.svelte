<script lang="ts">
	import subBadge from '$resources/sub.svelte';
	import moderatorBadge from '$resources/moderator.svelte';
	import partnerBadge from '$resources/partner.svelte';
	import broadcasterBadge from '$resources/broadcaster.svelte';
	import vipBadge from '$resources/vip.svelte';
	import Logger from '$lib/logger/log';
	import { Badge, GlobalBadgeCache } from '$lib/store/badges';
	import type { HelixChatBadgeVersion } from '@twurple/api';

	export let id: string;
	export let version: string;

	Logger.trace(`id ${id} version ${version}`);

	let badge: HelixChatBadgeVersion | undefined;

	if (GlobalBadgeCache.Has(id)) {
		badge = GlobalBadgeCache.Get(id)?.GetVersion(version) ?? undefined;
	}

	Logger.trace(badge);

	const badgeMap: Map<string, any> = new Map<string, any>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);
</script>

{#if badge}
	<img class="inline max-w-none h-5" src={badge.getImageUrl(4)} />
{:else}
	<svelte:component this={badgeMap.get(id)} />
{/if}
