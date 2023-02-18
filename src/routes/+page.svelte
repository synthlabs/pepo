<script>
	import { StaticAuthProvider } from '@twurple/auth';
	import { ChatClient } from "@twurple/chat";
	import { onDestroy } from 'svelte';
	import { animateScroll } from 'svelte-scrollto-element';
	import { beforeUpdate, afterUpdate, tick } from 'svelte';

	// username=the_xin;user_id=30403955;client_id=g5zg0400k4vhrx2g6xi4hgveruamlv;oauth_token=wbkhaqy1qyrwj2eguzx7jwmdq7xi4d;
	const clientId = 'g5zg0400k4vhrx2g6xi4hgveruamlv';
	const accessToken = 'wbkhaqy1qyrwj2eguzx7jwmdq7xi4d';

	const authProvider = new StaticAuthProvider(clientId, accessToken);

	const chatClient = new ChatClient({ authProvider, channels: ['hasanabi'] });

	chatClient.connect().then(()=> {console.log("connected")})

	chatClient.onMessage((channel, user, text, msg) => {
		let m = {
			ts: msg.date.toLocaleTimeString(),
			username: msg.userInfo.displayName,
			message: text,
		}
		messages = [...messages, m]
	})

	onDestroy(() => {
		chatClient.quit()
	});

	let messages = [
		{ ts: '00:00:00 pm', username: 'ImLazy', message: 'this is me being lazy' },
	];

	/**
	* @type {HTMLDivElement}
	*/
	let chatwindow;
	/**
	* @type {boolean}
	*/
	let autoscroll;

	beforeUpdate(() => {
		// determine whether we should auto-scroll
		// once the DOM is updated...
		if (chatwindow) {
			console.log(chatwindow.offsetHeight, chatwindow.scrollTop, chatwindow.scrollHeight)
			autoscroll = chatwindow && (chatwindow.offsetHeight + chatwindow.scrollTop) > (chatwindow.scrollHeight - 20);
		}
	});

	afterUpdate(() => {
		// ...the DOM is now in sync with the data
		if (autoscroll) chatwindow.scrollTo(0, chatwindow.scrollHeight);
	});

</script>

<style>
	.chat {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
</style>

<div class="chat">
	<h1>hi</h1>
	<div bind:this={chatwindow} style="overflow-y: auto;" class="p-2">
		{#each messages as msg}
			<div>
				<span class="text-primary text-opacity-80">{msg.ts}</span>
				<span class="text-secondary">{msg.username}</span><span>:</span>
				<span>{msg.message}</span>
			</div>
		{/each}
	</div>
	<input type="text">
</div>
