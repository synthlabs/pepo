<script lang="ts">
	import '../app.css';
	import { currentUser } from '$lib/store/runes/user.svelte';
	import { currentToken } from '$lib/store/runes/token.svelte';
	import { client } from '$lib/store/runes/apiclient.svelte';
	import {
		channels as channelCache,
		Decode as ccDecode,
		Encode as ccEncode
	} from '$lib/store/channels';
	import Nav from '$lib/components/+nav.svelte';
	import InputNav from '$lib/components/+inputnav.svelte';
	import { GlobalBadgeCache, loadGlobalBadges } from '$lib/store/badges';
	import { GlobalEmoteCache, loadGlobalEmotes } from '$lib/store/emotes';

	currentUser.useLocalStorage();
	currentToken.useLocalStorage();
	channelCache.useLocalStorage(ccDecode, ccEncode);

	client.token = currentToken;

	currentToken.validate().then((valid) => {
		if (valid) {
			loadGlobalBadges(client.api, GlobalBadgeCache);
			loadGlobalEmotes(client.api, GlobalEmoteCache);
		}
	});

	let modal: HTMLDialogElement;
	let inputNavInputReset: any;

	function crossPlatformActionModifier(e: KeyboardEvent): Boolean {
		const isMac = navigator.userAgent.includes('Mac');

		if (isMac) {
			return e.metaKey;
		}
		return e.ctrlKey;
	}

	function keyBind(node: HTMLElement) {
		function handleKeyPress(e: KeyboardEvent) {
			switch (e.key.toLowerCase()) {
				case 'k':
					if (crossPlatformActionModifier(e)) {
						modal.showModal();
						return;
					}
				// default:
				// 	Logger.debug(
				// 		`[KEYBIND] code=${e.code} key=${e.key} shift=${e.shiftKey} alt=${e.altKey} meta=${
				// 			e.metaKey
				// 		} ctrl=${e.ctrlKey} actionModifier=${crossPlatformActionModifier(e)}`
				// 	);
			}
		}
		window.addEventListener('keydown', handleKeyPress);
		return {
			destroy() {
				window.removeEventListener('keydown', handleKeyPress);
			}
		};
	}

	function inputNavSubmitted(e: CustomEvent) {
		modal.close();
	}
</script>

<div class="flex flex-col flex-nowrap w-full h-full" use:keyBind>
	<Nav />
	<slot />
</div>

<dialog id="channel-switcher" class="modal" bind:this={modal} on:close={inputNavInputReset}>
	<div class="modal-box">
		<InputNav bind:reset={inputNavInputReset} on:inputNavSubmitted={inputNavSubmitted} />
	</div>
</dialog>
