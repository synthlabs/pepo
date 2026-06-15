<script lang="ts">
	import BrandIcon from './brand-icon.svelte';

	let props = $props();
	const iconNode = [
		['rect', { width: 37.5, height: 112.5 }],
		['rect', { x: 37.5, y: 25, width: 12.5, height: 62.5 }],
		['rect', { x: 50, y: 12.5, width: 12.5, height: 87.5 }],
		['rect', { x: 62.5, width: 37.5, height: 37.5 }],
		['rect', { x: 62.5, y: 37.5, width: 25, height: 12.5 }],
		['rect', { x: 62.5, y: 50, width: 12.5, height: 62.5 }],
		['rect', { x: 75, y: 62.5, width: 12.5, height: 50 }],
		['rect', { x: 87.5, y: 75, width: 12.5, height: 37.5 }]
	];
</script>

<BrandIcon name="kick" title="Kick" viewBox="0 0 100 112.5" color="#53FC18" {...props} iconNode={iconNode} />
