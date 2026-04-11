<script lang="ts">
	import type { Emote } from '$lib/bindings';

	interface Props {
		emote: Emote;
	}

	let { emote }: Props = $props();
</script>

<img class="inline size-6" src={emote.url} alt={emote.name} />
