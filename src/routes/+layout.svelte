<script>
	import '../app.css';
	import { page } from '$app/state';

	let { children } = $props();

	$inspect(page.params.id);
</script>

<div class="h-full w-full">
	{@render children?.()}
</div>
