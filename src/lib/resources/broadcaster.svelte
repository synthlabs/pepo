<script lang="ts">
	interface Props {
		sizePx?: number;
	}

	let { sizePx = 20 }: Props = $props();
</script>

<span style="font-size: {sizePx}px; line-height: 1;">B</span>
