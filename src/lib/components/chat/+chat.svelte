<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy, onMount } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ApiClient, HelixStream } from '@twurple/api';
	import { parseChatMessage } from '@twurple/common';
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import { v4 as uuidv4 } from 'uuid';
	import { GlobalEmoteCache } from '$lib/store/emotes';
	import { chatClient } from '$lib/store/chat';
	import { TwitchToken, isValid, token } from '$lib/store/token';
	import Logger from '$lib/logger/log';
	import Badges from '$lib/components/chat/+badges.svelte';
	import * as types from '$lib/config/constants';
	import type {
		BasicParsedMessagePart,
		ParsedMessageTextPart
	} from '@twurple/common/lib/emotes/ParsedMessagePart';
	import { beforeNavigate, afterNavigate } from '$app/navigation';

	let div: HTMLDivElement;
	let autoscroll: boolean;

	const GREY_NAME_COLOR = '#6B7280';

	interface user {
		id: string;
		name: string;
		color: string;
	}

	interface message {
		id: string;
		ts: string;
		username: string;
		messageParts: BasicParsedMessagePart[];
		color: string;
		raw?: TwitchPrivateMessage;
	}
	let messages: message[] = [];

	export let channel: string;
	let currentUser: user;
	let behavior: ScrollBehavior = 'auto';
	let input = '';
	let streamInfo: Promise<HelixStream | null> = new Promise((res) => res(null));

	const toke = $token ? $token : new TwitchToken();

	let authProvider: StaticAuthProvider;
	let chat = $chatClient;
	let apiClient: ApiClient;
	$: if (isValid(toke)) {
		authProvider = new StaticAuthProvider(toke.client_id, toke.oauth_token);
		apiClient = new ApiClient({ authProvider });

		Logger.debug('valid token');
		streamInfo = getStream(channel);
		init();
	}

	onMount(() => {
		Logger.debug('mounted');
		chat.connect().then(() => {
			Logger.info('connected to chat');
		});

		chat.onMessage((msgChannel, _user, text, msg) => {
			if (msgChannel === `#${channel}`) {
				twitchMsgHandler(text, msg);
			} else {
				Logger.warn(
					`expected messages for ${channel} but got one for ${msgChannel} - trying to part again`
				);
				chat.part(msgChannel);
			}
		});

		chat.onJoin((channel, user) => {
			Logger.debug(`joined ${channel} as ${user}`);
		});
		chat.onPart((channel, user) => {
			Logger.debug(`parted ${channel} as ${user}`);
		});

		chat.join(channel);
	});

	beforeNavigate((_) => {
		Logger.debug(`navigating - leaving ${channel}`);
		chat.part(channel);
		Logger.debug('clearing msg cache');
		messages = [];
	});

	afterNavigate((_) => {
		Logger.debug(`navigated - joining ${channel}`);
		chat.join(channel);
		Logger.debug('clearing msg cache');
		messages = [];
	});

	onDestroy(() => {
		Logger.debug('destroyed - quitting chat');
		chat.quit();
	});

	async function getStream(userName: string): Promise<HelixStream | null> {
		const user = await apiClient.users.getUserByName(userName);
		if (!user) {
			Logger.debug('failed to get user');
			return null;
		}

		return await user.getStream();
	}

	async function init() {
		Logger.debug('init');

		let stream = await streamInfo;
		if (!stream) {
			Logger.error('failed to get stream info', stream);
			return;
		}

		GlobalEmoteCache.UseClient(apiClient);
		GlobalEmoteCache.LoadChannel(stream.userId);
	}

	function uptime(stream: HelixStream): number {
		return (Date.now() - stream.startDate.getTime()) / 1000;
	}

	function formatTime(seconds: number) {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.round(seconds % 60);
		return [h, m > 9 ? m : h ? '0' + m : m || '0', s > 9 ? s : '0' + s].filter(Boolean).join(':');
	}

	function twitchMsgHandler(text: string, msg: TwitchPrivateMessage) {
		let m: message;

		m = {
			id: msg.id,
			ts: msg.date.toLocaleTimeString('en', { timeStyle: 'short' }),
			username: msg.userInfo.displayName,
			messageParts: parseChatMessage(text, msg.emoteOffsets),
			color: msg.userInfo.color ?? GREY_NAME_COLOR,
			raw: msg
		};
		msgHandler(m);
	}

	function msgHandler(msg: message) {
		messages = [...messages, msg];
	}

	beforeUpdate(() => {
		// determine whether we should auto-scroll
		// once the DOM is updated...
		autoscroll = div && div.offsetHeight + div.scrollTop > div.scrollHeight - 20;
	});

	afterUpdate(() => {
		// ...the DOM is now in sync with the data
		if (autoscroll) {
			div.scrollTo({ top: div.scrollHeight, left: 0, behavior });
		}
	});

	$: if (isValid(toke)) {
		apiClient.getTokenInfo().then(async (token) => {
			let u: user = {
				id: token.userId ?? uuidv4(),
				name: token.userName ?? 'unknown',
				color: GREY_NAME_COLOR
			};
			u.color = (await apiClient.chat.getColorForUser(u.id)) ?? u.color;
			currentUser = u;
		});
	}

	$: hasInput = input.length > 0;

	const submitForm = (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement;

		if (hasInput && currentUser) {
			chat
				.say(channel, input)
				.catch(Logger.error)
				.finally(() => {
					input = '';
					if (event.target) {
						target.reset();
					}
				});

			let part: ParsedMessageTextPart = {
				type: 'text',
				text: input,
				position: 0,
				length: input.length
			};
			msgHandler({
				id: uuidv4(),
				ts: new Date().toLocaleTimeString('en', { timeStyle: 'short' }),
				username: currentUser.name,
				messageParts: [part],
				color: currentUser.color
			});
		}
	};
</script>

<div class="flex flex-col flex-nowrap w-full h-full p-2">
	<div class="flex p-1 border-b border-b-base-300">
		<div class="flex items-center normal-case text-xl pl-1 pr-1">#{channel}</div>
		{#await streamInfo}
			<div class="flex items-center pl-3 border-l-2 ml-2 text-sm" />
		{:then stream}
			{#if stream}
				<div class="flex items-center pl-3 border-l-2 ml-2 text-sm">{stream.title}</div>
			{:else}
				<div class="flex items-center pl-3 border-l-2 ml-2 text-sm">Offline</div>
			{/if}
		{/await}
	</div>

	<div class="flex-grow overflow-y-auto neg-horiz-p-2 text-sm" bind:this={div}>
		{#each messages as msg (msg.id)}
			<div
				class="even:bg-base-100 odd:bg-base-200 pl-2 pr-2 pt-1 pb-1 inline-block align-middle w-full"
			>
				<span class="text-xs text-gray-500 whitespace-nowrap">{msg.ts}</span>
				<Badges message={msg.raw} />
				<span class="whitespace-nowrap" style="color: {msg.color}; font-weight: 700;"
					>{msg.username}</span
				>:
				{#each msg.messageParts as m}
					{#if m.type == types.TEXT_TOKEN}
						{m.text}
					{:else if m.type == types.EMOTE_TOKEN}
						{#if GlobalEmoteCache.Has(m.id)}
							<img
								class="inline max-w-none"
								src={GlobalEmoteCache.passthroughGet(m.id)?.getStaticImageUrl('1.0', 'dark')}
								alt={m.name}
							/>
						{:else}
							[{m.name}]
						{/if}
					{:else if m.type == types.CHEER_TOKEN}
						[[{m.name}]]
					{/if}
				{/each}
			</div>
		{/each}
	</div>

	<span class="flex items-center justify-center">
		<!-- TODO: hide button first since there's a delay in the async scrollto -->
		<button
			on:click={() => div.scrollTo({ top: div.scrollHeight, left: 0, behavior })}
			class:hidden={autoscroll}
			class="absolute bottom-24 bg-slate-700 hover:bg-slate-600 p-2 rounded-lg items-center justify-center text-center text-xs"
		>
			Chat Paused - click to scroll
		</button>
	</span>

	<form on:submit|preventDefault={submitForm}>
		<div class="form-control p-1 border-t border-t-base-300">
			<div class="p-1 text-sm">
				{#await streamInfo then stream}
					{#if stream}
						{stream.viewers} viewers, {formatTime(uptime(stream))} uptime
					{/if}
				{/await}
			</div>
			<div class="relative">
				<!-- TODO: properly pad input like the password field so text doesn't go behind button -->
				<input
					bind:value={input}
					type="text"
					class="input w-full p-1 input-bordered focus:input-primary hover:input-primary"
					placeholder="Chat away..."
				/>
				<button
					type="submit"
					class="absolute inset-y-0 right-0 flex items-center pr-3 btn btn-ghost"
					style={hasInput ? '' : 'pointer-events: none;'}
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 24 24"
						stroke-width="1.5"
						class="w-6 h-6 fill-none"
						class:stroke-primary={hasInput}
						class:stroke-slate-600={!hasInput}
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5"
						/>
					</svg>
				</button>
			</div>
		</div>
	</form>
</div>

<style>
	.neg-horiz-p-2 {
		margin-left: -0.5rem;
		margin-right: -0.5rem;
	}
</style>
