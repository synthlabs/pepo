<script lang="ts">
	import '../app.css';
	import { user } from '$lib/store/user';
	import { TwitchToken } from '$lib/store/runes/token.svelte';
	import { client } from '$lib/store/runes/apiclient.svelte';
	import {
		channels as channelCache,
		Decode as ccDecode,
		Encode as ccEncode
	} from '$lib/store/channels';
	import Nav from '$lib/components/+nav.svelte';
	import InputNav from '$lib/components/+inputnav.svelte';
	import { GlobalBadgeCache } from '$lib/store/badges';
	import { GlobalEmoteCache } from '$lib/store/emotes';

	user.useLocalStorage();
	channelCache.useLocalStorage(ccDecode, ccEncode);

	let newTwitchToken = new TwitchToken();
	client.token = newTwitchToken;

	newTwitchToken.validate().then((valid) => {
		if (valid) {
			GlobalBadgeCache.UseClient(client.api);
			GlobalEmoteCache.UseClient(client.api);
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
