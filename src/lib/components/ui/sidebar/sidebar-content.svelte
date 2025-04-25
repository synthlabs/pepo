<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { WithElementRef } from 'bits-ui';
	import { cn } from '$lib/utils.js';

	let {
		ref = $bindable(null),
		class: className,
		scrollWhenCollapsed = true,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLElement>> & {
		scrollWhenCollapsed?: boolean;
	} = $props();
</script>

<div
	bind:this={ref}
	data-sidebar="content"
	class={cn(
		'flex min-h-0 flex-1 flex-col gap-2 overflow-auto',
		className,
		scrollWhenCollapsed ? 'icon-scroll' : 'group-data-[collapsible=icon]:overflow-hidden'
	)}
	{...restProps}
>
	{@render children?.()}
</div>

<style>
	.icon-scroll::-webkit-scrollbar {
		display: none;
	}
</style>
