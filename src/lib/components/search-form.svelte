<script lang="ts">
	import { Label } from '$lib/components/ui/label/index.ts';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Popover from '$lib/components/ui/popover/index.ts';
	import { Input } from '$lib/components/ui/input/index.ts';
	import { useSidebar } from '$lib/components/ui/sidebar/context.svelte.ts';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';

	interface Props {
		value?: string;
		onsubmit?: (channel: string) => void;
	}

	let { value = $bindable(''), onsubmit }: Props = $props();

	const sidebar = useSidebar();
	let popoverOpen = $state(false);

	function handleSubmit() {
		if (value.trim() && onsubmit) {
			onsubmit(value.trim().toLowerCase());
			value = '';
			popoverOpen = false;
		}
	}
</script>

{#snippet searchInput(id: string, inputClass?: string)}
	<div class="relative min-w-0 flex-1">
		<Label for={id} class="sr-only">Search</Label>
		<Input
			{id}
			placeholder="Chat..."
			class="h-8 ps-8 {inputClass ?? ''}"
			bind:value
		/>
		<SearchIcon
			class="pointer-events-none absolute inset-s-2 top-1/2 size-4 -translate-y-1/2 opacity-50 select-none"
		/>
		{#if value}
			<button
				type="button"
				onclick={() => (value = '')}
				class="absolute inset-e-2 top-1/2 size-4 -translate-y-1/2 cursor-pointer opacity-50 hover:opacity-100"
			>
				<XIcon class="size-4" />
				<span class="sr-only">Clear search</span>
			</button>
		{/if}
	</div>
{/snippet}

{#if sidebar.state === 'collapsed'}
	<Sidebar.Group class="py-0">
		<Sidebar.GroupContent class="flex justify-center">
			<Popover.Root bind:open={popoverOpen}>
				<Popover.Trigger
					class="flex size-8 items-center justify-center rounded-md hover:bg-sidebar-accent"
					onpointerenter={() => (popoverOpen = true)}
				>
					<SearchIcon class="size-4 opacity-50" />
					<span class="sr-only">Search</span>
				</Popover.Trigger>
				<Popover.Content
					side="right"
					align="start"
					class="w-64 p-2"
					onpointerleave={() => (popoverOpen = false)}
				>
					<form
						onsubmit={(e) => {
							e.preventDefault();
							handleSubmit();
						}}
					>
						{@render searchInput('search-floating')}
					</form>
				</Popover.Content>
			</Popover.Root>
		</Sidebar.GroupContent>
	</Sidebar.Group>
{:else}
	<form
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
	>
		<Sidebar.Group class="py-0">
			<Sidebar.GroupContent class="flex gap-1">
				{@render searchInput('search')}
			</Sidebar.GroupContent>
		</Sidebar.Group>
	</form>
{/if}
