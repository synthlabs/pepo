<script lang="ts">
	import { goto, beforeNavigate } from '$app/navigation';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';

	import '../app.css';
	import { user } from '$lib/store/user';
	import { isValid, token } from '$lib/store/token';
	import Logger from '$lib/logger/log';
	import Nav from '$lib/components/+nav.svelte';

	user.useLocalStorage();
	token.useLocalStorage();

	$: Logger.debug('user: ', $user);
	$: Logger.debug('token: ', $token);

	onMount(() => {
		if (!$token || !isValid($token)) {
			Logger.warn('no valid token');
			goto('/settings');
		}
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
