<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ChatClient } from "@twurple/chat";
	import { ClientID, AccessToken } from "$lib/config/config";
	import { ApiClient, HelixStream } from '@twurple/api';
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import type { ParsedMessagePart } from '@twurple/common';

	let div: HTMLDivElement;
	let autoscroll: boolean;

	let channel = "hasanabi";
	let behavior: ScrollBehavior = "auto";
	let input = "";

	const authProvider = new StaticAuthProvider(ClientID, AccessToken);
	const chatClient = new ChatClient({ authProvider, channels: [channel] });
	const apiClient = new ApiClient({ authProvider });

	async function getStream(userName: string): Promise<HelixStream | null> {
		const user = await apiClient.users.getUserByName(userName);
		if (!user) {
			return null;
		}

		return await user.getStream();
	}

	function formatTime(seconds: number) {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.round(seconds % 60);
		return [
			h,
			m > 9 ? m : (h ? '0' + m : m || '0'),
			s > 9 ? s : '0' + s
		].filter(Boolean).join(':');
	}

	let streamInfo = getStream(channel)

	chatClient.connect().then(()=> {console.log("connected")})
	onDestroy(() => {
		chatClient.quit()
	});

	interface message {
		ts: string;
		username: string;
		message: string;
		raw: TwitchPrivateMessage;
	}
	let messages: message[] = [];

	chatClient.onMessage((_channel, _user, _text, msg) => {
		let constructedText = ""
		msg.parseEmotes().forEach((curr, _i, _full) => {
			switch (curr.type) {
				case "text":
					constructedText += curr.text
					return
				case "emote":
				case "cheer":
					constructedText += `[${curr.name}]`
					return
			}
		})

		let m = {
			ts: msg.date.toLocaleTimeString("en", {timeStyle: "short"}),
			username: msg.userInfo.displayName,
			message: constructedText,
			raw: msg,
		}
		messages = [...messages, m]

		

		console.log(constructedText)

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

	$: console.log(input.length > 0)

</script>

<style>

</style>

<div class="flex flex-col h-full p-2">
	<div class="p-1 border-b-2">
		{#await streamInfo then stream}
			<span class="normal-case text-xl">#{channel}</span>
			<span class="pl-3 border-l-2 ml-2">{stream?.title}</span>
		{/await}
	</div>
	
	<div class="flex-1 overflow-y-auto p-2 text-sm" bind:this={div}>
		{#each messages as msg (msg)}
			<div>
				<span class="text-xs text-gray-500">{msg.ts}</span>
				<span class="text-secondary">{msg.username}</span><span>:</span>
				<span>{msg.message}</span>
			</div>
		{/each}
	</div>

	<div class="form-control p-1">
		<div class="p-1 text-xs">
			{#await streamInfo then stream}
				{stream?.viewers} viewers, {formatTime((Date.now() - stream?.startDate)/1000)} uptime 
			{/await}
		</div>
		<div class="relative">
			<input bind:value={input} type="text" class="input w-full p-1 input-bordered focus:input-primary" placeholder="Chat away...">
			<div class="absolute inset-y-0 right-0 flex items-center pr-3 btn btn-ghost">
				<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
					<path stroke-linecap="round" stroke-linejoin="round" d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5" />
				</svg>
			</div>
		</div>
	</div>
</div>