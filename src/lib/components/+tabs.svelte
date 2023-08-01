<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import Logger from '$lib/logger/log';
	import { channels as channelCache, Sanitize } from '$lib/store/channels';

	let inputStr = '';
	let modalOpen = false;
	let modal: HTMLDialogElement;
	let channels: string[] = [];

	$: borders = generateBorders(channels, $page.params.channel);

	channelCache.subscribe((cache) => {
		channels = Array.from(cache);
	});

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

	function closeTab(name: string) {
		// TODO: if we're on the tab they just closed, we should go to the next tab
		let index = channels.indexOf(name) ?? 0;

		$channelCache.delete(name);
		channelCache.set($channelCache);

		const redirect = channels[Math.min(index, channels.length - 1)];
		console.log(`closed ${name} tab - cache ${Array.from($channelCache)} - redirect ${redirect}`);
		if (redirect) {
			goto(`/chat/${redirect}`);
		} else {
			goto('/');
		}
	}

	function clickTab(name: string) {
		goto(`/chat/${name}`);
	}

	function generateBorders(channels: string[], current: string): Boolean[] {
		const size = channels.length;
		const borders: Boolean[] = new Array(size);

		for (var i = 0; i < size; i++) {
			if (channels[i] == current) {
				borders[i] = false;
			} else {
				const next = i + 1;
				const hasNext = next < size;

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

<!-- tabs placeholder for when they're hidden -->
<div class="flex-row flex-grow items-end sm:hidden h-full gap-1" />
<!-- tabs -->
<div class="flex-row flex-grow items-end xxs:tabs hidden h-full gap-1">
	{#each channels as name, i}
		<!-- tab -->
		<!-- svelte-ignore a11y-no-static-element-interactions -->
		<div
			class="tab tab-lifted ellipsis max-w-xs"
			class:tab-active={name === $page.params.channel}
			class:tab-border={borders[i]}
			on:click={() => clickTab(name)}
		>
			<a on:click|preventDefault|stopPropagation={() => clickTab(name)} href="/chat/{name}"
				>{name}</a
			>
			<!-- svelte-ignore a11y-no-static-element-interactions -->
			<div
				on:click|stopPropagation={(e) => closeTab(name)}
				class="absolute inset-y-0 right-0 flex items-center pr-2 cursor-pointer"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
					stroke-width="2"
					class="w-4 h-4 stroke-slate-500 hover:stroke-slate-300"
				>
					<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</div>
		</div>
	{/each}
	<button
		on:click={openDialog}
		class="flex items-center justify-center text-center plus cursor-pointer"
	>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			fill="none"
			viewBox="0 0 24 24"
			stroke-width="2"
			class="w-5 h-5 stroke-slate-400 hover:stroke-slate-300"
		>
			<path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m6-6H6" />
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

<style lang="postcss">
	.ellipsis {
		flex: 1;
		min-width: 4rem;
		/* or some value */
	}
	.ellipsis a {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
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
		height: 2.25rem;
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

	.tab {
		@apply relative inline-flex cursor-pointer select-none flex-wrap items-center justify-center text-center;
		@apply h-9 text-sm leading-loose;
		--tab-padding: 1rem;

		padding-left: var(--tab-padding, 1rem);
		padding-right: var(--tab-padding, 1rem);

		@apply text-opacity-50 [@media(hover:hover)]:hover:text-opacity-100;
		--tab-color: hsl(var(--bc) / var(--tw-text-opacity, 1));
		--tab-bg: hsl(var(--b1) / var(--tw-bg-opacity, 1));
		--tab-border-color: hsl(var(--b3) / var(--tw-bg-opacity, 1));
		color: var(--tab-color);
		&.tab-active:not(.tab-disabled):not([disabled]) {
			@apply border-base-content border-opacity-100 text-opacity-100;
		}
		&:focus {
			@apply outline-none;
		}
		&:focus-visible {
			outline: 2px solid currentColor;
			outline-offset: -3px;
			&.tab-lifted {
				border-bottom-right-radius: var(--tab-radius, 0.5rem);
				border-bottom-left-radius: var(--tab-radius, 0.5rem);
			}
		}
		/* disabled */
		&-disabled,
		&[disabled] {
			@apply text-base-content text-opacity-20 cursor-not-allowed;
		}
		@media (hover: hover) {
			&[disabled],
			&[disabled]:hover {
				@apply text-base-content text-opacity-20 cursor-not-allowed;
			}
		}
	}
	.tab-lifted {
		border: var(--tab-border, 1px) solid transparent;
		border-width: 0 0 var(--tab-border, 1px) 0;
		border-top-left-radius: var(--tab-radius, 0.5rem);
		border-top-right-radius: var(--tab-radius, 0.5rem);
		border-bottom-color: var(--tab-border-color);
		padding-left: 1rem;
		padding-right: 2.05rem;
		padding-top: var(--tab-border, 1px);
		&.tab-active:not(.tab-disabled):not([disabled]) {
			background-color: var(--tab-bg);
			border-width: var(--tab-border, 1px) var(--tab-border, 1px) 0 var(--tab-border, 1px);
			border-left-color: var(--tab-border-color);
			border-right-color: var(--tab-border-color);
			border-top-color: var(--tab-border-color);
			padding-left: calc(1rem - var(--tab-border, 1px));
			padding-right: calc(2.05rem - var(--tab-border, 1px));
			padding-bottom: var(--tab-border, 1px);
			padding-top: 0;
			&:before,
			&:after {
				z-index: 1;
				content: '';
				display: block;
				position: absolute;
				width: var(--tab-radius, 0.5rem);
				height: var(--tab-radius, 0.5rem);
				bottom: 0;
				--tab-grad: calc(68% - var(--tab-border, 1px));
				--tab-corner-bg: radial-gradient(
					circle at var(--circle-pos),
					transparent var(--tab-grad),
					var(--tab-border-color) calc(var(--tab-grad) + 0.3px),
					var(--tab-border-color) calc(var(--tab-grad) + var(--tab-border, 1px)),
					var(--tab-bg) calc(var(--tab-grad) + var(--tab-border, 1px) + 0.3px)
				);
			}
			&:before {
				left: calc(var(--tab-radius, 0.5rem) * -1);
				--circle-pos: top left;
				background-image: var(--tab-corner-bg);
			}
			&:after {
				right: calc(var(--tab-radius, 0.5rem) * -1);
				--circle-pos: top right;
				background-image: var(--tab-corner-bg);
			}
			&:first-child:before {
				background: none;
			}
			&:last-child:after {
				background: none;
			}
		}
	}
	.tab-lifted.tab-active:not(.tab-disabled):not([disabled])
		+ .tab-lifted.tab-active:not(.tab-disabled):not([disabled]) {
		&:before {
			background: none;
		}
	}
</style>
