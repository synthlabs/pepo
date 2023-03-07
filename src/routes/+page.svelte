<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ChatClient } from "@twurple/chat";
	import {ClientID, AccessToken} from "$lib/config/config";
	import { Cog6Tooth } from '@steeze-ui/heroicons'
	import { Icon } from '@steeze-ui/svelte-icon'

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

<div class="flex flex-col h-full p-2">
	<div class="p-1">
		<span class="normal-case text-xl">#{channel}</span>
	</div>
	

	<div class="divider"></div> 

	<div class="flex-1 overflow-y-auto" bind:this={div}>
		{#each messages as msg}
			<div>
				<span class="text-primary text-opacity-80">{msg.ts}</span>
				<span class="text-secondary">{msg.username}</span><span>:</span>
				<span>{msg.message}</span>
			</div>
		{/each}
	</div>

	<div class="form-control p-1">
		<div class="p-1">
			<span class="flex-1">some info</span>
		</div>
		<div class="relative">
			<input type="text" class="input w-full p-1 input-bordered focus:border-0 focus:input-primary" placeholder="Write your love letter here...">
			<div class="absolute inset-y-0 right-0 flex items-center pr-3">
				<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
					<path stroke-linecap="round" stroke-linejoin="round" d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5" />
				</svg>
			</div>
		</div>
	</div>
</div>