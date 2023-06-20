<script lang="ts">
	import { page } from '$app/stores';
	import Logger from '$lib/logger/log';
	import { channels as channelCache, Sanitize } from '$lib/store/channels';

	channelCache.useLocalStorage();

	let inputStr = '';
	let modalOpen = false;
	let channels = $channelCache ? $channelCache : [];
	let modal: HTMLDialogElement;

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
			channels = [...channels, chan];
			channelCache.set(channels);
			closeDialog();
		}
	}

	function openDialog() {
		modal.showModal();
	}

	function closeDialog() {
		inputStr = '';
		modal.close();
	}
</script>

<svelte:window on:keyup={handleEscape} />

<div class="tabs flex-shrink-0 bg-base-100 w-full gap-1 pl-1 pt-1 pr-1">
	{#each channels as name}
		<a class="tab" class:tab-active={name === $page.params.channel} href="/chat/{name}">{name}</a>
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
				placeholder="Add a new favorite channel"
			/>
		</form>
	</div>
</dialog>

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
