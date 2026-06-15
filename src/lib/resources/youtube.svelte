<script lang="ts">
	import BrandIcon from './brand-icon.svelte';

	let props = $props();
	const iconNode = [
		['rect', { width: 24, height: 18, rx: 4.4, fill: '#FF0000' }],
		['path', { d: 'M9.6 4.9 16.4 9 9.6 13.1Z', fill: '#fff' }]
	];
</script>

<BrandIcon name="youtube" title="YouTube" viewBox="0 0 24 18" {...props} iconNode={iconNode} />
