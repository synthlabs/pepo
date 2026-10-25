<script lang="ts">
	import { AlertDialog } from 'bits-ui';
	import AppWindow from '@lucide/svelte/icons/app-window';
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import Smile from '@lucide/svelte/icons/smile';
	import Search from '@lucide/svelte/icons/search';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import {
		SETTINGS_CATEGORIES,
		SETTINGS_CONTROLS,
		matchingControls,
		resetSettingsPatch,
		type SettingsCategory
	} from '$lib/settings/controls';
	import { settingsState, updateAppSettings } from '$lib/stores/settings.svelte';
	import SettingRow from './setting-row.svelte';

	let selected = $state<SettingsCategory>('chat');
	let query = $state('');
	let more = $state<Record<SettingsCategory, boolean>>({ chat: false, emotes: false, app: false });
	let resetOpen = $state(false);
	let resetTarget = $state<SettingsCategory | 'all'>('chat');
	let resetVersion = $state(0);
	let category = $derived(SETTINGS_CATEGORIES.find((item) => item.id === selected)!);
	let searching = $derived(query.trim().length > 0);
	let basicControls = $derived(
		SETTINGS_CONTROLS.filter((control) => control.category === selected && !control.more)
	);
	let extraControls = $derived(
		SETTINGS_CONTROLS.filter((control) => control.category === selected && control.more)
	);
	let results = $derived(matchingControls(query));
	let resetLabel = $derived(
		resetTarget === 'all'
			? 'all settings'
			: `${SETTINGS_CATEGORIES.find((item) => item.id === resetTarget)!.label} settings`
	);

	function requestReset(target: SettingsCategory | 'all') {
		resetTarget = target;
		resetOpen = true;
	}
	function reset() {
		void updateAppSettings(resetSettingsPatch(resetTarget));
		resetVersion += 1;
	}
</script>

<div class="settings-shell h-full min-h-0 min-w-0 flex-1" data-settings-page>
	<div class="settings-layout flex h-full min-h-0 flex-col">
		<aside class="settings-navigation flex shrink-0 flex-col gap-4 border-b p-4">
			<div class="relative">
				<Search
					class="text-muted-foreground pointer-events-none absolute top-2.5 left-2.5 size-4"
				/>
				<Input
					type="search"
					bind:value={query}
					aria-label="Search settings"
					placeholder="Search settings…"
					class="h-9 pl-8 text-sm"
				/>
			</div>
			<nav class="settings-categories flex gap-1" aria-label="Settings categories">
				{#each SETTINGS_CATEGORIES as item (item.id)}
					<Button
						variant={!searching && selected === item.id ? 'secondary' : 'ghost'}
						size="sm"
						class="justify-start gap-2.5"
						aria-current={!searching && selected === item.id ? 'page' : undefined}
						onclick={() => {
							selected = item.id;
							query = '';
						}}
					>
						{#if item.id === 'chat'}<MessageSquare />{:else if item.id === 'emotes'}<Smile
							/>{:else}<AppWindow />{/if}
						{item.label}
					</Button>
				{/each}
			</nav>
		</aside>
		<section
			class="min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain px-4 py-6 @min-[640px]/settings:px-7"
			aria-labelledby="settings-category-title"
		>
			<div class="mx-auto flex max-w-3xl flex-col gap-5">
				<div class="flex flex-wrap items-start justify-between gap-4">
					<div class="min-w-0">
						<h2 id="settings-category-title" class="text-lg font-medium">
							{searching ? 'Search results' : category.label}
						</h2>
						<p class="text-muted-foreground mt-1 text-sm">
							{searching ? `${results.length} matching settings` : category.description}
						</p>
					</div>
					<div class="inline-flex shrink-0" role="group" aria-label="Reset settings">
						<Button
							variant="outline"
							size="sm"
							class="h-8 rounded-r-none border-r-0 px-2.5 text-xs"
							disabled={!settingsState.ready}
							onclick={() => requestReset(selected)}
							><RotateCcw class="size-3.5" />Reset {category.label}</Button
						>
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class={buttonVariants({
									variant: 'outline',
									size: 'icon',
									class: 'h-8 w-8 rounded-l-none'
								})}
								aria-label="More reset actions"
								disabled={!settingsState.ready}
								><ChevronDown class="size-3.5" /></DropdownMenu.Trigger
							>
							<DropdownMenu.Content align="end"
								><DropdownMenu.Item onclick={() => requestReset('all')}
									>Reset all settings</DropdownMenu.Item
								></DropdownMenu.Content
							>
						</DropdownMenu.Root>
					</div>
				</div>
				{#if !settingsState.ready}<p class="text-muted-foreground text-sm" role="status">
						Loading settings…
					</p>{/if}
				{#key resetVersion}
					{#if searching}
						{#if results.length > 0}
							<div class="divide-y overflow-hidden rounded-lg border">
								{#each results as control (control.id)}<SettingRow {control} showCategory />{/each}
							</div>
						{:else}<p class="text-muted-foreground rounded-lg border px-4 py-8 text-center text-sm">
								No settings match “{query.trim()}”.
							</p>{/if}
					{:else}
						<div class="divide-y overflow-hidden rounded-lg border">
							{#each basicControls as control (control.id)}<SettingRow {control} />{/each}
						</div>
						{#if extraControls.length > 0}
							<Button
								variant="ghost"
								size="sm"
								class="text-muted-foreground self-start px-0 hover:bg-transparent"
								aria-expanded={more[selected]}
								aria-controls={`settings-more-${selected}`}
								onclick={() => {
									more[selected] = !more[selected];
								}}
							>
								{#if more[selected]}<ChevronDown />{:else}<ChevronRight />{/if}
								{more[selected] ? 'Hide extra options' : `More options (${extraControls.length})`}
							</Button>
							{#if more[selected]}<div
									id={`settings-more-${selected}`}
									class="divide-y overflow-hidden rounded-lg border"
								>
									{#each extraControls as control (control.id)}<SettingRow {control} />{/each}
								</div>{/if}
						{/if}
					{/if}
				{/key}
			</div>
		</section>
	</div>
</div>

<AlertDialog.Root bind:open={resetOpen}>
	<AlertDialog.Portal>
		<AlertDialog.Overlay class="fixed inset-0 z-50 bg-black/50" />
		<AlertDialog.Content
			class="bg-background fixed top-1/2 left-1/2 z-50 w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border p-6 shadow-lg"
		>
			<AlertDialog.Title class="text-lg font-semibold">Reset {resetLabel}?</AlertDialog.Title>
			<AlertDialog.Description class="text-muted-foreground mt-2 text-sm leading-relaxed"
				>{resetTarget === 'all'
					? 'Restore Chat, Emotes, and App preferences to their defaults.'
					: `Restore ${SETTINGS_CATEGORIES.find((item) => item.id === resetTarget)!.label} preferences to their defaults.`}
				Changes save automatically.</AlertDialog.Description
			>
			<div class="mt-6 flex justify-end gap-2">
				<AlertDialog.Cancel class={buttonVariants({ variant: 'outline', size: 'sm' })}
					>Cancel</AlertDialog.Cancel
				>
				<AlertDialog.Action class={buttonVariants({ size: 'sm' })} onclick={reset}
					>Reset to defaults</AlertDialog.Action
				>
			</div>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>

<style>
	.settings-shell {
		container: settings / inline-size;
	}
	@container settings (min-width: 640px) {
		.settings-layout {
			flex-direction: row;
		}
		.settings-navigation {
			width: 220px;
			border-bottom: 0;
			border-right-width: 1px;
		}
		.settings-categories {
			flex-direction: column;
		}
	}
</style>
