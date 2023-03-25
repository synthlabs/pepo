<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ChatClient } from "@twurple/chat";
	import { ClientID, AccessToken } from "$lib/config/config";
	import { ApiClient, HelixStream } from '@twurple/api';
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import { v4 as uuidv4 } from 'uuid';

	let div: HTMLDivElement;
	let autoscroll: boolean;

	export let channel: string;
	let currentUser = "";
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

    function uptime(stream: HelixStream | null): number {
        let start = stream?.startDate;

        if (start) {
            return (Date.now() - start.getTime())/1000
        }

        return 0
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
		id: string;
		ts: string;
		username: string;
		message: string;
		color: string;
		raw?: TwitchPrivateMessage;
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


		console.log(msg.id, msg.userInfo)
		let m = {
			id: msg.id,
			ts: msg.date.toLocaleTimeString("en", {timeStyle: "short"}),
			username: msg.userInfo.displayName,
			message: constructedText,
			color: msg.userInfo.color ?? "#6B7280",
			raw: msg,
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

	apiClient.getTokenInfo().then((token) => {
		currentUser = token.userName ?? ""
	})

	$: hasInput = input.length > 0

	const submitForm = (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement

		chatClient.say(channel, input).catch(console.log).finally(() => {
			input = ""
			if (event.target) { target.reset() }
		})

		if (currentUser != "") {
			let m = {
				id: uuidv4(),
				ts: new Date().toLocaleTimeString("en", {timeStyle: "short"}),
				username: currentUser,
				message: input,
				color: "#6419E6",
			}
			messages = [...messages, m]
		}
	};

</script>

<style>

.neg-horiz-p-2 {
	margin-left: -0.5rem;
	margin-right: -0.5rem;
}

</style>

<div class="flex flex-col h-full p-2">
	<div class="p-1 border-b border-b-base-300">
		{#await streamInfo then stream}
			<span class="normal-case text-xl">#{channel}</span>
			<span class="pl-3 border-l-2 ml-2">{stream?.title}</span>
		{/await}
	</div>
	
	<div class="flex-1 overflow-y-auto neg-horiz-p-2 text-sm" bind:this={div}>
		{#each messages as msg (msg.id)}
			<div class="even:bg-base-100 odd:bg-base-200 pl-2 pr-2 pt-1 pb-1">
				<span class="text-xs text-gray-500">{msg.ts}</span>
				<span style="color: {msg.color}; font-weight: 700;">{msg.username}</span><span>:</span>
				<span>{msg.message}</span>
			</div>
		{/each}
	</div>

	<form on:submit|preventDefault={submitForm}>
		<div class="form-control p-1 border-t border-t-base-300">
			<div class="p-1 text-sm">
				{#await streamInfo then stream}
					{stream?.viewers} viewers, {formatTime(uptime(stream))} uptime
				{/await}
			</div>
			<div class="relative">
				<input bind:value={input} type="text" class="input w-full p-1 input-bordered focus:input-primary hover:input-primary" placeholder="Chat away...">
				<div class="absolute inset-y-0 right-0 flex items-center pr-3 btn btn-ghost">
					<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" stroke-width="1.5" class="w-6 h-6 fill-none" class:stroke-primary={hasInput} class:stroke-current={!hasInput}>
						<path stroke-linecap="round" stroke-linejoin="round" d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5" />
					</svg>
				</div>
			</div>
		</div>
	</form>
</div>