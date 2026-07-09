<script lang="ts">
	import type { Emote } from '$lib/bindings';

	interface Props {
		emote: Emote;
		showTier?: boolean;
	}

	let { emote, showTier = true }: Props = $props();
</script>

<div class="flex flex-wrap items-center justify-center gap-3" style="max-width: min(92vw, 320px);">
	<div class="flex max-h-[72px] max-w-[112px] shrink-0 items-center justify-center overflow-hidden">
		<img
			class="block h-auto max-h-[72px] w-auto max-w-[112px] object-contain"
			src={emote.url}
			alt={emote.name}
		/>
	</div>
	<div class="flex max-w-[240px] min-w-0 flex-1 basis-48 flex-col gap-0.5 text-left">
		<p class="leading-snug font-semibold wrap-anywhere">{emote.name}</p>
		<p class="text-muted-foreground text-xs wrap-anywhere">
			{emote.provider} · {emote.scope}
		</p>
		{#if showTier && emote.tier}
			<p class="text-muted-foreground text-xs wrap-anywhere">
				Tier: {emote.tier}
			</p>
		{/if}
	</div>
</div>
