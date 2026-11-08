<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { mergeProps } from 'bits-ui';
	import House from '@lucide/svelte/icons/house';
	import { Button } from '$lib/components/ui/button';
	import * as Sidebar from '$lib/components/ui/sidebar';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import SearchForm from '$lib/components/search-form.svelte';
	import { cn } from '$lib/utils';

	let {
		value = $bindable(''),
		onsubmit
	}: { value?: string; onsubmit?: (channel: string) => void } = $props();

	const sidebar = Sidebar.useSidebar();
	let searchFocused = $state(false);
	let isHome = $derived(page.url.pathname === resolve('/app'));
	let showHome = $derived(
		!isHome && !(searchFocused && (sidebar.state === 'expanded' || sidebar.isMobile))
	);
</script>

<Sidebar.Header
	class={cn(
		'relative h-12 gap-0 p-0 transition-[height] duration-200 ease-linear',
		isHome ? 'group-data-[collapsible=icon]:h-24' : 'group-data-[collapsible=icon]:h-36'
	)}
>
	<div
		data-sidebar-search
		class={cn(
			'absolute top-2 left-2 min-w-0 transition-[top,right] duration-200 ease-linear group-data-[collapsible=icon]:right-2',
			showHome
				? 'right-22 group-data-[collapsible=icon]:top-26'
				: 'right-12 group-data-[collapsible=icon]:top-14'
		)}
		onfocusin={() => (searchFocused = true)}
		onfocusout={(event) => {
			if (!(
				event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)
			)) {
				searchFocused = false;
			}
		}}
	>
		<SearchForm bind:value {onsubmit} />
	</div>
	{#if showHome}
		<div
			class="absolute top-2 right-12 flex size-8 items-center justify-center group-data-[collapsible=icon]:top-13 group-data-[collapsible=icon]:right-0 group-data-[collapsible=icon]:left-0 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-auto"
		>
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Button
							{...mergeProps(props, { onclick: () => (value = '') })}
							href={resolve('/app')}
							variant="ghost"
							size="icon"
							aria-label="Home"
							data-sidebar-home
							class="group-data-[collapsible=icon]:hover:bg-sidebar-accent group-data-[collapsible=icon]:hover:text-sidebar-accent-foreground group-data-[collapsible=icon]:active:bg-sidebar-accent group-data-[collapsible=icon]:active:text-sidebar-accent-foreground group-data-[collapsible=icon]:focus-visible:ring-sidebar-ring size-8 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-full group-data-[collapsible=icon]:rounded-lg group-data-[collapsible=icon]:focus-visible:ring-offset-0"
						>
							<House />
						</Button>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content
					side={sidebar.state === 'collapsed' && !sidebar.isMobile ? 'right' : 'bottom'}
					>Home</Tooltip.Content
				>
			</Tooltip.Root>
		</div>
	{/if}
	<div
		class="absolute top-2 right-2 flex size-8 items-center justify-center group-data-[collapsible=icon]:top-1 group-data-[collapsible=icon]:right-0 group-data-[collapsible=icon]:left-0 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-auto"
	>
		<Sidebar.Trigger
			class="group-data-[collapsible=icon]:hover:bg-sidebar-accent group-data-[collapsible=icon]:hover:text-sidebar-accent-foreground group-data-[collapsible=icon]:active:bg-sidebar-accent group-data-[collapsible=icon]:active:text-sidebar-accent-foreground group-data-[collapsible=icon]:focus-visible:ring-sidebar-ring size-8 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-full group-data-[collapsible=icon]:rounded-lg group-data-[collapsible=icon]:focus-visible:ring-offset-0"
		/>
	</div>
</Sidebar.Header>
