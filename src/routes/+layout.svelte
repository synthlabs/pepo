<script lang="ts">
	import { goto, beforeNavigate } from '$app/navigation';
	import { onMount } from 'svelte';
	import '../app.css';
	import { user } from '$lib/store/user';
	import { chatClient } from '$lib/store/chat';
	import { isValid, token, validate as validateToken } from '$lib/store/token';
	import {
		channels as channelCache,
		Decode as ccDecode,
		Encode as ccEncode
	} from '$lib/store/channels';
	import Logger from '$lib/logger/log';
	import Nav from '$lib/components/+nav.svelte';
	import InputNav from '$lib/components/+inputnav.svelte';

	user.useLocalStorage();
	token.useLocalStorage();
	channelCache.useLocalStorage(ccDecode, ccEncode);

	let modal: HTMLDialogElement;
	let inputNavInputReset: any;

	$: if ($token) {
		Logger.debug('token updated');
		validateToken($token)
			.then((t) => {
				if (!isValid(t) && $token.isValid) {
					$token.isValid = false;
					Logger.warn('no valid token');
					return;
				}

				if (isValid(t)) {
					$chatClient.token = $token;
				}
			})
			.catch(Logger.error);
	}

	$: Logger.debug('user: ', $user);

	onMount(() => {
		validateToken($token)
			.then((t) => {
				$token = t;
			})
			.catch(Logger.error);
	});

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
