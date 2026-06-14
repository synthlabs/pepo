<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';

	interface Props {
		emote: Emote;
		sizePx?: number;
	}

	let { emote, sizePx = 28 }: Props = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		<img
			class="inline max-w-none"
			style="height: {sizePx}px; min-width: {sizePx}px;"
			src={emote.url}
			alt={emote.name}
		/>
	</Tooltip.Trigger>
	<Tooltip.Content class="flex items-center gap-3 p-2">
		<div class="flex h-[80px] w-[80px] shrink-0 items-center justify-center">
			<img
				class="inline max-w-none"
				style="height: 80px;"
				src={emote.url}
				alt={emote.name}
			/>
		</div>
		<div class="flex flex-col gap-0.5">
			<p class="font-semibold">{emote.name}</p>
			<p class="text-xs text-muted-foreground">{emote.provider} · {emote.scope}</p>
			{#if emote.tier}
				<p class="text-xs text-muted-foreground">Tier: {emote.tier}</p>
			{/if}
		</div>
	</Tooltip.Content>
</Tooltip.Root>