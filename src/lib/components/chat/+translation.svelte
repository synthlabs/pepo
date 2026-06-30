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
		<span class="relative inline-block whitespace-nowrap align-baseline">
			<span aria-hidden="true" class="invisible">
				<span class="font-bold">{authorName}</span>:{' '}
			</span>
			<span class="text-primary absolute top-0 right-2 font-mono text-xs font-semibold whitespace-nowrap">
				{source} -> {target}
			</span>
		</span>
		<span
			class="text-muted-foreground text-[0.8125rem] leading-snug break-words text-wrap align-baseline [overflow-wrap:anywhere]"
			>{text}</span
		>
	{:else}
		<span
			class="text-muted-foreground inline-flex max-w-full min-w-0 gap-2 text-[0.8125rem] leading-snug align-baseline"
		>
			<span class="text-primary shrink-0 font-mono text-xs font-semibold whitespace-nowrap">
				{source} -> {target}
			</span>
			<span class="min-w-0 break-words text-wrap [overflow-wrap:anywhere]">{text}</span>
		</span>
	{/if}
{/if}
