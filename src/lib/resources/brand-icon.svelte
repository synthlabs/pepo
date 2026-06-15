<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { SVGAttributes } from 'svelte/elements';

	type IconNode = [tag: string, attrs: Record<string, string | number>][];

	interface Props extends SVGAttributes<SVGSVGElement> {
		name?: string;
		size?: number | string;
		color?: string;
		title?: string;
		viewBox?: string;
		iconNode: IconNode;
		children?: Snippet;
	}

	let {
		name,
		size = 24,
		color = 'currentColor',
		title,
		viewBox = '0 0 24 24',
		iconNode,
		children,
		class: className,
		...props
	}: Props = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	{viewBox}
	{...props}
	width={size}
	height={size}
	fill={color}
	role={title ? 'img' : undefined}
	aria-hidden={title ? undefined : 'true'}
	aria-label={title}
	class={['brand-icon', name && `brand-icon-${name}`, className]}
>
	{#if title}
		<title>{title}</title>
	{/if}
	{#each iconNode as [tag, attrs]}
		<svelte:element this={tag} {...attrs} />
	{/each}
	{@render children?.()}
</svg>
