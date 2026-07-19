<script lang="ts">
	import type { ChannelMessageTranslation, ChatTranslationLayout } from '$lib/bindings';

	interface Props {
		translation: ChannelMessageTranslation | null;
		authorName: string;
		layout: ChatTranslationLayout;
	}

	let { translation, authorName, layout }: Props = $props();
	let source = $derived(translation?.source_language.trim().toUpperCase() ?? '');
	let target = $derived(translation?.target_language.trim().toUpperCase() ?? '');
	let text = $derived(translation?.translated_text.trim() ?? '');
</script>

{#if text}
	{#if layout === 'message_text'}
		<span class="relative inline-block align-baseline whitespace-nowrap">
			<span aria-hidden="true" class="invisible">
				<span class="font-bold">{authorName}</span>:&#32;
			</span>
			<span
				class="text-primary absolute top-0 right-2 font-mono text-[0.75rem] font-semibold whitespace-nowrap"
			>
				{source} -> {target}
			</span>
		</span>
		<span
			class="text-muted-foreground align-baseline text-[0.75rem] leading-snug text-wrap wrap-anywhere"
			>{text}</span
		>
	{:else}
		<span
			class="text-muted-foreground align-baseline text-[0.75rem] leading-snug text-wrap wrap-anywhere"
		>
			{#if layout === 'connector'}
				<span class="inline-flex items-baseline whitespace-nowrap">
					<span class="inline-flex pr-2 pl-1">
						<span
							aria-hidden="true"
							data-translation-connector
							class="border-primary inline-block h-2.5 w-[18px] -translate-y-0.5 rounded-bl-xl border-b-2 border-l-2 align-baseline"
						></span>
					</span>
					<span class="text-primary pr-1 font-mono text-[0.75rem] font-semibold">
						{source} -> {target}
					</span>
				</span>{:else}
				<span class="text-primary font-mono text-[0.75rem] font-semibold whitespace-nowrap">
					{source} -> {target}
				</span>{/if}
			<span>{text}</span>
		</span>
	{/if}
{/if}
