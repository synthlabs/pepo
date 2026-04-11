<script lang="ts">
	import { Label } from '$lib/components/ui/label/index.ts';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import PlusIcon from '@lucide/svelte/icons/plus';

	interface Props {
		value?: string;
		onsubmit?: (channel: string) => void;
	}

	let { value = $bindable(''), onsubmit }: Props = $props();

	function handleJoin() {
		if (value.trim() && onsubmit) {
			onsubmit(value.trim().toLowerCase());
			value = '';
		}
	}
</script>

<form
	onsubmit={(e) => {
		e.preventDefault();
		handleJoin();
	}}
>
	<Sidebar.Group class="py-0">
		<Sidebar.GroupContent class="flex gap-1">
			<div class="relative min-w-0 flex-1">
				<Label for="search" class="sr-only">Search</Label>
				<Sidebar.Input
					id="search"
					placeholder="Chat..."
					class="min-w-0 ps-8 transition-opacity duration-200 ease-linear group-data-[collapsible=icon]:opacity-0"
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
			<button
				type="button"
				onclick={handleJoin}
				class="border-input bg-background hover:bg-sidebar-accent flex h-8 w-8 shrink-0 items-center justify-center rounded-md border group-data-[collapsible=icon]:hidden"
			>
				<PlusIcon class="size-4" />
				<span class="sr-only">Join channel</span>
			</button>
		</Sidebar.GroupContent>
	</Sidebar.Group>
</form>
