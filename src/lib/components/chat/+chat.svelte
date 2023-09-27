<script lang="ts">
	import { beforeUpdate, afterUpdate, onDestroy, onMount } from 'svelte';
	import { StaticAuthProvider } from '@twurple/auth';
	import { ApiClient, HelixStream, HelixUser } from '@twurple/api';
	import { parseChatMessage } from '@twurple/common';
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import { v4 as uuidv4 } from 'uuid';
	import { GlobalEmoteCache } from '$lib/store/emotes';
	import { GlobalBadgeCache } from '$lib/store/badges';
	import { chatClient } from '$lib/store/chat';
	import { IsAnonUser, user } from '$lib/store/user';
	import { isValid, token } from '$lib/store/token';
	import Logger from '$lib/logger/log';
	import Badges from '$lib/components/chat/+badges.svelte';
	import * as types from '$lib/config/constants';
	import type {
		BasicParsedMessagePart,
		ParsedMessageTextPart
	} from '@twurple/common/lib/emotes/ParsedMessagePart';
	import { beforeNavigate, afterNavigate } from '$app/navigation';
	import { channels as channelCache } from '$lib/store/channels';
	import { getTwitchEmoteURL } from '$lib/util/twitch';
	import { BrowserCache } from '$lib/chat/cache';

	const GREY_NAME_COLOR = '#6B7280';
	const AUTOSCROLL_BUFFER = 200; // the amount you can scroll up and still not disable auto scroll

	export let channel: string;

	let div: HTMLDivElement;
	let autoscroll: boolean;
	let autoscrollDebounce: boolean;
	let chatInput: HTMLInputElement;
	let authProvider: StaticAuthProvider;
	let messageCache: BrowserCache<message>;

	let messageLimit = 1000;
	let behavior: ScrollBehavior = 'auto';
	let input = '';
	let streamInfo: HelixStream | null;
	let index = 0;
	let streamRefreshInterval: NodeJS.Timeout;

	interface message {
		id: string;
		ts: string;
		username: string;
		messageParts: BasicParsedMessagePart[];
		color: string;
		index: number;
		raw?: TwitchPrivateMessage;
	}
	let messages: message[] = [];

	// TODO: move this into a global client like the chat client
	let apiClient: ApiClient;
	$: if (isValid($token)) {
		authProvider = new StaticAuthProvider($token.client_id, $token.oauth_token);
		apiClient = new ApiClient({ authProvider });

		Logger.debug('valid token');

		getStream(channel).then((info) => {
			streamInfo = info;
		});
		init();
	}

	$: hasInput = input.length > 0;

	onMount(() => {
		messageCache = new BrowserCache();
		Logger.debug('mounted');
	});

	onDestroy(() => {
		Logger.debug('destroyed');
	});

	beforeNavigate((_) => {
		Logger.debug(`navigating - unsubscribing from ${channel}`);
		$chatClient.unsub(channel);

		Logger.debug(`navigating - persisting msg cache for ${channel}`);
		messageCache.set(channel, [...messages]);
		messages = [];

		Logger.debug(`navigating - clearing stream refresh interval`);
		clearInterval(streamRefreshInterval);
	});

	afterNavigate((_) => {
		Logger.debug(`navigated - ${channel}`);
		$channelCache = $channelCache.add(channel);

		clearInterval(streamRefreshInterval);
		streamRefreshInterval = setInterval(async () => {
			Logger.debug('get stream tick');
			streamInfo = await getStream(channel);
		}, 60000);

		Logger.debug(`navigated - loading msg cache for ${channel}`);
		messages = messageCache.get(channel);

		Logger.debug(`navigated - subscribing to ${channel}`);
		$chatClient.sub(channel, twitchMsgHandler);

		autoscrollDebounceFn();
	});

	beforeUpdate(() => {
		if (autoscrollDebounce) {
			autoscroll = true;
			return;
		}

		const scrollAmount = div.offsetHeight + div.scrollTop;
		// determine whether we should auto-scroll
		// once the DOM is updated...
		// if the scroll amount matches the tail minus a buffer amount then autoscroll
		autoscroll = div && scrollAmount > div.scrollHeight - AUTOSCROLL_BUFFER;
	});

	afterUpdate(() => {
		// ...the DOM is now in sync with the data
		if (autoscroll) {
			div.scrollTo({ top: div.scrollHeight, left: 0, behavior });
		}
	});

	const init = async () => {
		Logger.debug('init');

		let user = await getUserByName(channel);
		if (!user) {
			Logger.error('failed to get user info', channel);
			return;
		}

		GlobalBadgeCache.LoadChannel(channel, user.id);
		GlobalEmoteCache.LoadChannel(user.id);
	};

	const getUserByName = async (userName: string): Promise<HelixUser | null> => {
		const user = await apiClient.users.getUserByName(userName);
		if (!user) {
			Logger.debug('failed to get user');
			return null;
		}
		return user;
	};

	const getStream = async (userName: string): Promise<HelixStream | null> => {
		const user = await apiClient.users.getUserByName(userName);
		if (!user) {
			Logger.debug('failed to get user');
			return null;
		}
		return await user.getStream();
	};

	const uptime = (stream: HelixStream): number => {
		return (Date.now() - stream.startDate.getTime()) / 1000;
	};

	const formatTime = (seconds: number) => {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.round(seconds % 60);
		return [h, m > 9 ? m : h ? '0' + m : m || '0', s > 9 ? s : '0' + s].filter(Boolean).join(':');
	};

	const twitchMsgHandler = (text: string, msg: TwitchPrivateMessage) => {
		let m: message;

		m = {
			id: msg.id,
			ts: msg.date.toLocaleTimeString('en', { timeStyle: 'short' }),
			username: msg.userInfo.displayName,
			messageParts: parseChatMessage(text, msg.emoteOffsets),
			color: msg.userInfo.color ?? GREY_NAME_COLOR,
			index: nextIndex(),
			raw: msg
		};
		msgHandler(m);
	};

	const msgHandler = (msg: message) => {
		let newMsgs = [...messages, msg];

		if (newMsgs.length > messageLimit) newMsgs.shift();

		messages = newMsgs;
	};

	const submitForm = (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement;

		if (hasInput) {
			$chatClient
				.say(channel, input)
				.catch(Logger.error)
				.finally(() => {
					input = '';
					if (event.target) {
						target.reset();
					}
				});
		}
	};

	const keyRedirect = (node: HTMLInputElement) => {
		function handleKeydown(e: KeyboardEvent) {
			switch (e.key.toLowerCase()) {
				case 'escape':
					node.blur();
					return;
			}

			if (node === document.activeElement || document.activeElement?.tagName === 'INPUT') {
				return;
			}

			if (e.key.match(/^\w$/g)) {
				node.focus();
			}
		}
		window.addEventListener('keydown', handleKeydown);
		return {
			destroy() {
				window.removeEventListener('keydown', handleKeydown);
			}
		};
	};

	const autoscrollDebounceFn = () => {
		autoscrollDebounce = true;
		Logger.trace('autoscroll debounce START');
		setTimeout(() => {
			Logger.trace('autoscroll debounce FINISH');
			autoscrollDebounce = false;
		}, 2000);
	};

	const pausedChatBtnAction = () => {
		autoscrollDebounceFn();
		div.scrollTo({ top: div.scrollHeight, left: 0, behavior });
	};

	const nextIndex = (): number => {
		index = index + 1;
		return index;
	};

	const evenOddClass = (x: number): string => {
		//even:bg-base-100 odd:bg-base-200
		//background-color: hsl(var(--b1) / var(--tw-bg-opacity))
		if (x % 2 === 0) {
			return 'background-color: hsl(212 18% 14%)';
		}
		return 'background-color: hsl(213 18% 12%)';
	};
</script>

<div class="flex flex-col flex-nowrap w-full h-full p-2">
	<div class="flex p-1 border-b border-b-base-300">
		<!-- TODO: now that I have real tabs, this should be something else -->
		<div class="flex items-center normal-case text-xl pl-1 pr-1">#{channel}</div>

		{#if streamInfo}
			<div class="flex items-center pl-3 border-l-2 ml-2 text-sm">{streamInfo.title}</div>
		{:else if IsAnonUser($user, $token)}
			<div class="flex items-center pl-3 border-l-2 ml-2 text-sm">Unknown</div>
		{:else}
			<div class="flex items-center pl-3 border-l-2 ml-2 text-sm">Offline</div>
		{/if}
	</div>

	<div class="flex-grow overflow-y-auto neg-horiz-p-2 text-sm" bind:this={div}>
		{#each messages as msg (msg.id)}
			<div
				class="pl-2 pr-2 pt-1 pb-1 inline-block align-middle w-full"
				style={evenOddClass(msg.index)}
			>
				<span class="text-xs text-gray-500 whitespace-nowrap">{msg.ts}</span>
				<Badges message={msg.raw} {channel} />
				<span class="whitespace-nowrap" style="color: {msg.color}; font-weight: 700;"
					>{msg.username}</span
				>:
				{#each msg.messageParts as m}
					{#if m.type == types.TEXT_TOKEN}
						{m.text}
					{:else if m.type == types.EMOTE_TOKEN}
						<div class="tooltip" data-tip={m.name}>
							{#if GlobalEmoteCache.Has(m.id)}
								<img
									class="inline max-w-none h-6"
									src={GlobalEmoteCache.passthroughGet(m.id)?.getStaticImageUrl('3.0', 'dark')}
									alt={m.name}
								/>
							{:else}
								<img
									class="inline max-w-none h-6"
									src={getTwitchEmoteURL(m.id, 3.0)}
									alt={m.name}
								/>
							{/if}
						</div>
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
			on:click={pausedChatBtnAction}
			class:hidden={autoscroll}
			class="absolute bottom-24 bg-slate-700 hover:bg-slate-600 p-2 rounded-lg items-center justify-center text-center text-xs"
		>
			Chat Paused - click to scroll
		</button>
	</span>

	<form on:submit|preventDefault={submitForm}>
		<div class="form-control p-1 border-t border-t-base-300">
			<div class="p-1 text-sm">
				{#if streamInfo}
					{streamInfo.viewers} viewers, {formatTime(uptime(streamInfo))} uptime
				{/if}
			</div>
			<div class="relative">
				<!-- TODO: properly pad input like the password field so text doesn't go behind button -->
				<!-- TODO: use custom input and input bordered classes to get darker BW outline default, small primary when focused, full outline with text-->
				<input
					bind:value={input}
					bind:this={chatInput}
					disabled={IsAnonUser($user, $token)}
					use:keyRedirect
					type="text"
					class="w-full input input-bordered focus:input-primary hover:input-primary"
					style="padding-right: 3rem;"
					placeholder="Chat away..."
					tabindex="0"
				/>
				<button
					type="submit"
					class="absolute inset-y-0 right-0 flex items-center pr-3 cursor-pointer"
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
