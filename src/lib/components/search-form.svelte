<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import { Label } from '$lib/components/ui/label/index.ts';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Popover from '$lib/components/ui/popover/index.ts';
	import { Input } from '$lib/components/ui/input/index.ts';
	import { useSidebar } from '$lib/components/ui/sidebar/context.svelte.ts';
	import { sidebarMenuButtonVariants } from '$lib/components/ui/sidebar/sidebar-menu-button.svelte';
	import { cn } from '$lib/utils';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';

	interface Props {
		value?: string;
		onsubmit?: (channel: string) => void;
	}

	let { value = $bindable(''), onsubmit }: Props = $props();

	const sidebar = useSidebar();
	let popoverOpen = $state(false);
	let inputRef = $state<HTMLInputElement | null>(null);
	let clearRef = $state<HTMLButtonElement | null>(null);
	let triggerRef = $state<HTMLButtonElement | null>(null);
	let popoverRef = $state<HTMLDivElement | null>(null);
	let isFloatingSearch = $derived(sidebar.visualState === 'collapsed' && !sidebar.isMobile);
	let isMoving = $derived(
		!sidebar.isMobile &&
			(sidebar.visualState === 'expanding' || sidebar.visualState === 'collapsing')
	);
	let previousFloating: boolean | undefined;
	let handingOffPopover = false;
	let destroyed = false;
	let focusRequest = 0;

	$effect.pre(() => {
		const floating = isFloatingSearch;
		const moving = isMoving;
		untrack(() => {
			const active = document.activeElement;
			const replacingControl = previousFloating !== undefined && previousFloating !== floating;
			const ownsFocus =
				active !== null &&
				(active === inputRef ||
					active === clearRef ||
					active === triggerRef ||
					popoverRef?.contains(active));
			if (replacingControl && !floating) {
				handingOffPopover = true;
				popoverOpen = false;
			}
			previousFloating = floating;

			if ((replacingControl && ownsFocus) || (moving && clearRef && active === clearRef)) {
				const request = ++focusRequest;
				void tick().then(() => {
					if (destroyed || request !== focusRequest) return;
					if (document.activeElement !== active && document.activeElement !== document.body) return;
					(isFloatingSearch ? triggerRef : inputRef)?.focus();
				});
			}
		});
	});

	onDestroy(() => {
		destroyed = true;
	});

	function handlePopoverOpenChange(open: boolean) {
		if (open) handingOffPopover = false;
		else if (!handingOffPopover && isFloatingSearch) value = '';
	}

	function handlePopoverInteractOutside(event: PointerEvent) {
		if (
			event.target instanceof Element &&
			event.target.closest('[data-sidebar="trigger"], [data-sidebar="rail"]')
		) {
			event.preventDefault();
		}
	}

	function handlePopoverCloseAutoFocus(event: Event) {
		if (handingOffPopover) event.preventDefault();
	}

	function clearSearch() {
		value = '';
		inputRef?.focus();
	}

	function handleInputKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape' || isFloatingSearch) return;
		event.preventDefault();
		event.stopPropagation();
		value = '';
	}

	function handleSubmit() {
		if (value.trim() && onsubmit) {
			onsubmit(value.trim().toLowerCase());
			value = '';
			popoverOpen = false;
		}
	}
</script>

{#snippet searchInput(id: string)}
	<div class="relative min-w-0 flex-1">
		<Label for={id} class="sr-only">Search</Label>
		<Input
			{id}
			type="search"
			placeholder="Chat..."
			class="h-8 min-w-0 [&::-webkit-search-cancel-button]:hidden {isMoving
				? 'px-0 text-transparent caret-transparent selection:bg-transparent selection:text-transparent placeholder:text-transparent'
				: 'ps-8 pe-8'}"
			readonly={isMoving}
			autocomplete="off"
			autocapitalize="none"
			spellcheck={false}
			onkeydown={handleInputKeydown}
			bind:ref={inputRef}
			bind:value
		/>
		<SearchIcon
			class="pointer-events-none absolute inset-s-2 top-1/2 size-4 -translate-y-1/2 opacity-50 select-none"
		/>
		{#if value && !isMoving}
			<button
				bind:this={clearRef}
				type="button"
				onclick={clearSearch}
				class="absolute inset-e-2 top-1/2 size-4 -translate-y-1/2 cursor-pointer opacity-50 hover:opacity-100"
			>
				<XIcon class="size-4" />
				<span class="sr-only">Clear search</span>
			</button>
		{/if}
	</div>
{/snippet}

{#if isFloatingSearch}
	<Sidebar.Group class="py-0">
		<Sidebar.GroupContent class="flex justify-center">
			<Popover.Root bind:open={popoverOpen} onOpenChange={handlePopoverOpenChange}>
				<Popover.Trigger
					bind:ref={triggerRef}
					class={cn(
						sidebarMenuButtonVariants({ size: 'lg' }),
						'-mx-2 -my-1 w-[calc(100%+1rem)] shrink-0 justify-center'
					)}
				>
					<SearchIcon class="size-4 opacity-50" />
					<span class="sr-only">Search</span>
				</Popover.Trigger>
				<Popover.Content
					bind:ref={popoverRef}
					side="right"
					align="start"
					class="w-64 p-2"
					onInteractOutside={handlePopoverInteractOutside}
					onCloseAutoFocus={handlePopoverCloseAutoFocus}
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
