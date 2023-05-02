<script lang="ts">
	import { goto, beforeNavigate } from '$app/navigation';
	import { page } from '$app/stores';

	import '../app.css';
	import { user } from '$lib/store/user';
	import { isValid, token } from '$lib/store/token';
	import Logger from '$lib/logger/log';
	import { onMount } from 'svelte';

	user.useLocalStorage();
	token.useLocalStorage();

	$: Logger.debug('user: ', $user);
	$: Logger.debug('token: ', $token);

	onMount(() => {
		if (!$token || !isValid($token)) {
			Logger.warn('no valid token');
			goto('/login');
		}
	});

	beforeNavigate(({ from, to, cancel }) => {
		if (to?.route.id !== '/login' && !isValid($token)) {
			Logger.warn('token is not valid anymore');
			goto('/login');
		}
	});
</script>

<slot />
