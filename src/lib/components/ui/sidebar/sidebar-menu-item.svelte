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
	class={cn('group/menu-item relative', className, isActive ? 'border-r-2 border-primary' : '')}
	{...restProps}
>
	{@render children?.()}
</li>
