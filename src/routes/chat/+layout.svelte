<script lang="ts">
	import { page } from '$app/stores';

	const channels = [
		{ name: 'nmplol' },
		{ name: 'hasanabi' },
		{ name: 'sodapoppin' },
		{ name: 'itswill' }
	];
</script>

<div class="tabs bg-base-100 w-full">
	{#each channels as { name }}
		<a class="tab" class:tab-active={name === $page.params.channel} href="/chat/{name}">{name}</a>
	{/each}
	<div class="inline-flex flex-wrap items-center justify-center text-center plus cursor-pointer">
		<svg
			xmlns="http://www.w3.org/2000/svg"
			fill="none"
			viewBox="0 0 24 24"
			stroke-width="1.5"
			stroke="currentColor"
			class="w-6 h-6"
		>
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				d="M12 9v6m3-3H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z"
			/>
		</svg>
	</div>
</div>
<div class="flex flex-grow overflow-hidden w-full">
	<slot />
</div>

<style>
	.tab {
		border-radius: 0.5rem /* 8px */;
		--tw-bg-opacity: 1;
		background-color: hsl(var(--b2, var(--b1)) / var(--tw-bg-opacity));
	}
	.tab-active {
		--tw-bg-opacity: 1;
		background-color: hsl(var(--p) / var(--tw-bg-opacity));
	}
	.plus {
		height: 2rem /* 32px */;
		font-size: 0.875rem /* 14px */;
		line-height: 1.25rem /* 20px */;
		line-height: 2;
		--tab-padding: 0.5rem /* 16px */;
		--tw-text-opacity: 0.5;
		--tab-color: hsla(var(--bc) / var(--tw-text-opacity, 1));
		--tab-bg: hsla(var(--b1) / var(--tw-bg-opacity, 1));
		--tab-border-color: hsla(var(--b3) / var(--tw-bg-opacity, 1));
		color: var(--tab-color);
		padding-left: var(--tab-padding, 1rem);
		padding-right: var(--tab-padding, 1rem);
	}
</style>
