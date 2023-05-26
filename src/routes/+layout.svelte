<script lang="ts">
	import { goto, beforeNavigate } from '$app/navigation';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { ChatClient } from '@twurple/chat';
	import { StaticAuthProvider } from '@twurple/auth';
	import '../app.css';
	import { user } from '$lib/store/user';
	import { chatClient } from '$lib/store/chat';
	import { isValid, token } from '$lib/store/token';
	import Logger from '$lib/logger/log';
	import Nav from '$lib/components/+nav.svelte';

	user.useLocalStorage();
	token.useLocalStorage();

	$: if ($token) {
		Logger.debug('token updated: ', $token);
		const authProvider = new StaticAuthProvider($token.client_id, $token.oauth_token);
		chatClient.set(new ChatClient({ authProvider }));
	}

	$: Logger.debug('user: ', $user);

	onMount(() => {
		if (!$token || !isValid($token)) {
			Logger.warn('no valid token');
			goto('/settings');
		}
		const authProvider = new StaticAuthProvider($token.client_id, $token.oauth_token);
		chatClient.set(new ChatClient({ authProvider }));
	});

	beforeNavigate(({ from, to, cancel }) => {
		if (to?.route.id !== '/settings' && !isValid($token)) {
			Logger.warn('token is not valid anymore');
			goto('/settings');
		}
	});
</script>

<div class="flex flex-col flex-nowrap w-full h-full">
	<Nav />
	<slot />
</div>
