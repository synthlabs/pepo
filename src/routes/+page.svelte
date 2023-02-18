<script lang="ts">
	import Eliza from 'elizabot';
	import { beforeUpdate, afterUpdate } from 'svelte';

	let div: HTMLDivElement;
	let autoscroll: boolean;

	let channel = "#hasanabi";

	beforeUpdate(() => {
		// determine whether we should auto-scroll
		// once the DOM is updated...
		autoscroll = div && (div.offsetHeight + div.scrollTop) > (div.scrollHeight - 20);
	});

	afterUpdate(() => {
		// ...the DOM is now in sync with the data
		if (autoscroll) div.scrollTo(0, div.scrollHeight);
	});

	const eliza = new Eliza();

	let comments = [
		{ author: 'eliza', text: eliza.getInitial(), placeholder: false}
	];

	function handleKeydown(event: { key: string; target: { value: string; }; }) {
		if (event.key === 'Enter') {
			const text = event.target.value;
			if (!text) return;

			comments = comments.concat({
				author: 'user',
				text,
				placeholder: false,
			});

			event.target.value = '';

			const reply = eliza.transform(text);

			setTimeout(() => {
				comments = comments.concat({
					author: 'eliza',
					text: '...',
					placeholder: true
				});

				setTimeout(() => {
					comments = comments.filter(comment => !comment.placeholder).concat({
						author: 'eliza',
						text: reply,
						placeholder: false,
					});
				}, 500 + Math.random() * 500);
			}, 200 + Math.random() * 200);
		}
	}
</script>

<style>

</style>

<div class="flex flex-col h-full">
	<h1>{channel}</h1>

	<div class="flex-1 overflow-y-auto" bind:this={div}>
		<article>
			<span>How Do you do you bitch</span>
		</article>
	</div>

	<input on:keydown={handleKeydown} placeholder="write your love letter here">
</div>