<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { Button } from '$lib/components/ui/button';
	import { SETTINGS_CONTROLS } from '$lib/settings/controls';
	import type { TranslationSession } from '$lib/translation/session.svelte';
	import { modelInstallationProgress } from '$utils/translate/ui';
	import SettingRow from './setting-row.svelte';

	let {
		session,
		onadvanced,
		statusOnly = false
	}: { session?: TranslationSession; onadvanced: () => void; statusOnly?: boolean } = $props();
	const controls = SETTINGS_CONTROLS.filter(
		(control) => control.category === 'translation' && !control.more
	);
	const operation = $derived(session?.snapshot.installation);
	const active = $derived(operation && !operation.finished);
	const preparingTarget = $derived(
		active || session?.confirmation ? null : (session?.setupTarget ?? null)
	);
	let showPreparing = $state(false);
	$effect(() => {
		showPreparing = false;
		if (preparingTarget === null) return;
		const timer = setTimeout(() => {
			showPreparing = true;
		}, 300);
		return () => clearTimeout(timer);
	});
	const progress = $derived(operation ? modelInstallationProgress(operation) : undefined);
	const status = $derived(
		session?.snapshot.cancelling
			? 'Cancelling download…'
			: operation?.progress.phase === 'downloading'
				? 'Downloading translation support…'
				: 'Setting up translation…'
	);
</script>

{#if !statusOnly}<div
		class="translation-preferences divide-y overflow-hidden rounded-lg border"
		data-translation-preferences
	>
		{#each controls as control (control.id)}
			<SettingRow {control} translationSession={session} />
		{/each}
	</div>{/if}
{#if active && session}
	<div
		class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-x-4 gap-y-2"
		data-translation-setup
	>
		<div class="flex items-center justify-between gap-3 text-sm" role="status">
			<span>{status}</span>
			{#if progress !== undefined}<span class="tabular-nums">{Math.floor(progress)}%</span>{/if}
		</div>
		<Button
			variant="outline"
			size="sm"
			class="row-span-3"
			disabled={session.snapshot.cancelling}
			onclick={() => {
				session.abandonSetup();
				void session.controller.cancelInstallation();
			}}
			data-translation-cancel>{session.snapshot.cancelling ? 'Cancelling…' : 'Cancel'}</Button
		>
		<progress
			class="translation-progress h-2 w-full"
			value={progress}
			max="100"
			aria-label="Current translation setup phase progress"
		></progress>
		{#if session.setupTarget !== null}<p class="text-muted-foreground text-sm">
				{session.setupIntent === 'enable'
					? 'Translation will turn on when setup is complete.'
					: 'Your language will change when setup is complete.'}
			</p>{/if}
	</div>
{:else if preparingTarget !== null && showPreparing}
	<p class="text-muted-foreground text-sm" role="status">Preparing translation…</p>
{/if}
{#if session?.setupError}<p
		class="text-sm text-red-600 dark:text-red-400"
		role="alert"
		data-translation-setup-error
	>
		{session.setupError}
	</p>{/if}
{#if !statusOnly}<div class="mt-1">
		<Button
			variant="ghost"
			size="sm"
			class="-ml-2 gap-2 text-sm"
			onclick={onadvanced}
			data-open-translation-models
		>
			Advanced <ChevronRight class="size-4" />
		</Button>
		<p class="text-muted-foreground mt-1 text-sm">Additional translation settings.</p>
	</div>{/if}

<style>
	.translation-preferences :global([data-setting-row]) {
		flex-direction: row;
		align-items: center;
		justify-content: space-between;
	}
	.translation-preferences :global([data-setting-row] > :last-child) {
		max-width: 48%;
	}
	.translation-progress {
		appearance: none;
		overflow: hidden;
		border-radius: 9999px;
		background: hsl(var(--secondary));
		accent-color: hsl(var(--primary));
	}
	.translation-progress::-webkit-progress-bar {
		background: hsl(var(--secondary));
	}
	.translation-progress::-webkit-progress-value {
		background: hsl(var(--primary));
	}
	.translation-progress::-moz-progress-bar {
		background: hsl(var(--primary));
	}
</style>
