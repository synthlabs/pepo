<script lang="ts">
	import subBadge from '$lib/resources/sub.svelte';
	import moderatorBadge from '$lib/resources/moderator.svelte';
	import partnerBadge from '$lib/resources/partner.svelte';
	import broadcasterBadge from '$lib/resources/broadcaster.svelte';
	import vipBadge from '$lib/resources/vip.svelte';
	import type { BadgeRef } from '$lib/bindings';
	import type { Component } from 'svelte';
	import { Tooltip, TooltipTrigger, TooltipContent } from '$lib/components/ui/tooltip';
	import { badgeTooltipContent } from '$lib/chat/badge-tooltip';

	interface Props {
		badge_ref: BadgeRef;
		sizePx?: number;
	}

	let { badge_ref, sizePx = 20 }: Props = $props();

	let fallback = $derived(
		badge_ref.set_id !== badge_ref.badge.set_id || badge_ref.id !== badge_ref.badge.id
	);

	const badgeMap: Map<string, Component> = new Map<string, Component>([
		['subscriber', subBadge],
		['moderator', moderatorBadge],
		['partner', partnerBadge],
		['broadcaster', broadcasterBadge],
		['vip', vipBadge]
	]);

	const tooltipContent = $derived(badgeTooltipContent(badge_ref));
</script>

{#snippet badgeImage(renderSizePx: number)}
	{#if fallback}
		<!-- svelte-ignore svelte_component_deprecated -->
		<svelte:component this={badgeMap.get(badge_ref.set_id)} sizePx={renderSizePx} />
	{:else}
		<img
			class="inline max-w-none"
			style="height: {renderSizePx}px;"
			src={badge_ref.badge.image_url_4x}
			alt={badge_ref.badge.description}
		/>
	{/if}
{/snippet}

{#snippet tooltipBody()}
	<div class="p-2">
		<strong>{tooltipContent.name}</strong>
		{#if tooltipContent.description}
			<br />
			<span class="text-sm text-muted-foreground">{tooltipContent.description}</span>
		{/if}
		{#if tooltipContent.detail}
			<br />
			<span class="text-xs text-muted-foreground">{tooltipContent.detail}</span>
		{/if}
	</div>
{/snippet}

<Tooltip>
	<TooltipTrigger>
		{@render badgeImage(sizePx)}
	</TooltipTrigger>
	<TooltipContent class="flex items-center gap-3 p-2">
		<div class="flex h-[80px] w-[80px] shrink-0 items-center justify-center">
			{@render badgeImage(80)}
		</div>
		{@render tooltipBody()}
	</TooltipContent>
</Tooltip>
