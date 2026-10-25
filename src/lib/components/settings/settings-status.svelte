<script lang="ts">
	import Circle from '@lucide/svelte/icons/circle';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { settingsState } from '$lib/stores/settings.svelte';

	let label = $derived(
		!settingsState.ready
			? 'Loading settings…'
			: settingsState.result === 'error'
				? 'Save failed. Previous value restored.'
				: settingsState.saving
					? 'Saving settings…'
					: settingsState.result === 'saved'
						? 'Settings saved'
						: 'Changes save automatically'
	);
</script>

<Tooltip.Provider delayDuration={0}>
	<Tooltip.Root>
		<Tooltip.Trigger
			aria-label={label}
			class="focus-visible:ring-ring inline-flex size-7 cursor-default items-center justify-center rounded-md outline-none focus-visible:ring-2"
			data-settings-status
		>
			{#if settingsState.result === 'error'}
				<TriangleAlert class="text-destructive size-4" />
			{:else if !settingsState.ready || settingsState.saving}
				<LoaderCircle
					class="text-muted-foreground size-4 animate-spin motion-reduce:animate-none"
				/>
			{:else if settingsState.result === 'saved'}
				<CircleCheck class="size-4 text-emerald-600 dark:text-emerald-400" />
			{:else}
				<Circle class="text-muted-foreground size-3" />
			{/if}
		</Tooltip.Trigger>
		<Tooltip.Content data-settings-status-tooltip>{label}</Tooltip.Content>
	</Tooltip.Root>
</Tooltip.Provider>
<span class="sr-only" role="status" aria-live="polite" aria-atomic="true">{label}</span>
