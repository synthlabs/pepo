<script lang="ts">
	import { goto, beforeNavigate } from '$app/navigation';
	import { onMount } from 'svelte';
	import '../app.css';
	import { user } from '$lib/store/user';
	import { chatClient } from '$lib/store/chat';
	import { isValid, token } from '$lib/store/token';
	import { channels as channelCache } from '$lib/store/channels';
	import Logger from '$lib/logger/log';
	import Nav from '$lib/components/+nav.svelte';
	import InputNav from '$lib/components/+inputnav.svelte';

	user.useLocalStorage();
	token.useLocalStorage();
	channelCache.useLocalStorage();

	let modal: HTMLDialogElement;
	let inputNavInputReset: any;

	$: if ($token) {
		Logger.debug('token updated: ', $token);
		$chatClient.token = $token;
	}

	$: Logger.debug('user: ', $user);

	onMount(() => {
		if (!$token || !isValid($token)) {
			Logger.warn('no valid token');
			goto('/settings');
			return;
		}
		$chatClient.token = $token;
	});

	const freeRoutes = ['/', '/settings'];

	beforeNavigate(({ type, to, cancel }) => {
		Logger.debug(`nav type ${type}`);

		const isLeaving = type === 'leave';
		const isProtectedRoute = !freeRoutes.includes(to?.route.id ?? '');
		if (!isLeaving && isProtectedRoute && !isValid($token)) {
			Logger.warn('token is not valid anymore');
			cancel();
			goto('/settings');
		}
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
