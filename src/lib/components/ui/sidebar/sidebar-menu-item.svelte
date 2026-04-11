<script lang="ts">
	import { cn } from '$lib/utils.js';
	import type { WithElementRef } from 'bits-ui';
	import type { HTMLAttributes } from 'svelte/elements';

	let {
		ref = $bindable(null),
		class: className,
		isActive = false,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLLIElement>, HTMLLIElement> & {
		isActive?: boolean;
	} = $props();
</script>

<li
	bind:this={ref}
	data-sidebar="menu-item"
	class={cn('group/menu-item relative', className, isActive ? 'before:absolute before:inset-y-0 before:left-[-8px] before:w-1 before:rounded-r before:bg-ring before:transition-[left] before:duration-200 before:ease-linear group-data-[collapsible=icon]:before:left-0' : '')}
	{...restProps}
>
	{@render children?.()}
</li>
