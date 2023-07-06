<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import Logger from '$lib/logger/log';
	import { channels as channelCache, Sanitize } from '$lib/store/channels';

	channelCache.useLocalStorage();

	let inputStr = '';
	let modalOpen = false;
	let modal: HTMLDialogElement;

	$: borders = generateBorders($channelCache, $page.params.channel);

	// TODO: move this into a more central keybindings module
	// 		 https://www.reddit.com/r/sveltejs/comments/tp7hul/comment/i29svqt/

	function handleEscape(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			inputStr = '';
			modalOpen = false;
		}
	}

	function handleSave() {
		if (inputStr.length > 0) {
			const chan = Sanitize(inputStr);
			Logger.debug(chan);
			const channels = [...$channelCache, chan];
			channelCache.set(channels);
			closeDialog();
			goto(`/chat/${chan}`);
		}
	}

	function openDialog() {
		modal.showModal();
	}

	function closeDialog() {
		inputStr = '';
		modal.close();
	}

	function generateBorders(channels: string[], current: string): Boolean[] {
		const borders: Boolean[] = new Array(channels.length);

		for (var i = 0; i < channels.length; i++) {
			if (channels[i] == current) {
				borders[i] = false;
			} else {
				const next = i + 1;
				const hasNext = next < channels.length;

				if (hasNext && channels[next] == current) {
					borders[i] = false;
				} else {
					borders[i] = true;
				}
			}
		}
		return borders;
	}
</script>

<svelte:window on:keyup={handleEscape} />

<div class="tabs flex-shrink-0 w-full h-full gap-1">
	{#each $channelCache as name, i}
		<a
			class="tab tab-large tab-lifted"
			class:tab-active={name === $page.params.channel}
			class:tab-border={borders[i]}
			href="/chat/{name}">{name}</a
		>
	{/each}
	<button
		on:click={openDialog}
		class="inline-flex flex-wrap items-center justify-center text-center plus cursor-pointer"
	>
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
	</button>
</div>

<dialog id="add-channel" class="modal" bind:this={modal}>
	<div class="modal-box">
		<form method="dialog" on:submit|preventDefault={handleSave}>
			<input
				type="text"
				autofocus
				bind:value={inputStr}
				class="input input-bordered m-auto w-full input-sm"
				placeholder="Go to channel..."
			/>
		</form>
	</div>
</dialog>

<style>
	.tab-large {
		height: 2.5rem /* 48px */;
		font-size: 1rem /* 18px */;
		line-height: 1.75rem /* 28px */;
		line-height: 2;
		--tab-padding: 1.25rem /* 20px */;
	}

	.tab-border:after {
		content: '';
		position: absolute;
		right: 0px;
		top: 25%;
		height: 50%;
		border-right: 1px solid;
	}

	.plus {
		height: 2.5rem /* 32px */;
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
