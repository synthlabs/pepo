<script lang="ts">
	import type { Emote } from '$lib/bindings';

	interface Props {
		emote: Emote;
	}

	let { emote }: Props = $props();
</script>

<img class="inline h-7 min-w-7" src={emote.url} alt={emote.name} />
