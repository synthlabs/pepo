<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ChatClient } from "@twurple/chat";
	import {ClientID, AccessToken} from "$lib/config/config";

	let div: HTMLDivElement;
	let autoscroll: boolean;

	let channel = "hasanabi";
	let behavior: ScrollBehavior = "auto";

	const authProvider = new StaticAuthProvider(ClientID, AccessToken);
	const chatClient = new ChatClient({ authProvider, channels: [channel] });

	chatClient.connect().then(()=> {console.log("connected")})
	onDestroy(() => {
		chatClient.quit()
	});

	interface message {
		ts: string;
		username: string;
		message: string;
	}
	let messages: message[] = [];

	chatClient.onMessage((channel, user, text, msg) => {
		let m = {
			ts: msg.date.toLocaleTimeString(),
			username: msg.userInfo.displayName,
			message: text,
		}
		messages = [...messages, m]
	})

	beforeUpdate(() => {
		// determine whether we should auto-scroll
		// once the DOM is updated...
		autoscroll = div && (div.offsetHeight + div.scrollTop) > (div.scrollHeight - 20);
	});

	afterUpdate(() => {
		// ...the DOM is now in sync with the data
		if (autoscroll) {
			div.scrollTo({
				top: div.scrollHeight,
				left: 0,
				behavior,
			});
		}
	});	

</script>

<style>

</style>

<div class="flex flex-col h-full">
	<h1>#{channel}</h1>

	<div class="flex-1 overflow-y-auto" bind:this={div}>
		{#each messages as msg}
			<div>
				<span class="text-primary text-opacity-80">{msg.ts}</span>
				<span class="text-secondary">{msg.username}</span><span>:</span>
				<span>{msg.message}</span>
			</div>
		{/each}
	</div>

	<input placeholder="write your love letter here">
</div>