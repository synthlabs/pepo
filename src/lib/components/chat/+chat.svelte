<script lang="ts">
	import { onDestroy, onMount, tick } from 'svelte';
	import { HelixStream, HelixUser } from '@twurple/api';
	import { ChatEmote, parseChatMessage } from '@twurple/common';
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import { GlobalEmoteCache, loadChannelEmotes, loadGlobalEmotes } from '$lib/store/emotes';
	import { GlobalBadgeCache, loadChannelBadges } from '$lib/store/badges';
	import { chatClient } from '$lib/store/chat';
	import Logger from '$lib/logger/log';
	import Badges from '$lib/components/chat/+badges.svelte';
	import * as types from '$lib/config/constants';
	import type {
		BasicParsedMessagePart,
		ParsedMessageTextPart,
		ParsedMessageEmotePart
	} from '@twurple/common/lib/emotes/ParsedMessagePart';
	import { channels as channelCache } from '$lib/store/channels';
	import { getTwitchEmoteURL, getStream, getUserByName } from '$lib/util/twitch';
	import { BrowserCache } from '$lib/chat/cache';
	import { currentUser } from '$lib/store/runes/user.svelte';
	import { client } from '$lib/store/runes/apiclient.svelte';
	import { type Channel } from '$lib/types/twitch';

	const GREY_NAME_COLOR = '#6B7280';
	const AUTOSCROLL_BUFFER = 200; // the amount you can scroll up and still not disable auto scroll

	interface Props {
		name: string;
	}
	const { name }: Props = $props();
	let channel: Channel = $state({ name: name });

	let div: HTMLDivElement;
	let autoscroll: boolean = $state(true);
	let autoscrollDebounce: boolean = $state(true);
	let chatInput: HTMLInputElement;

	let behavior: ScrollBehavior = 'auto';
	let input = $state('');
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
	let messageLimit = 1000;
	let messages: message[] = $state([]);

	let hasInput = $derived(input.length > 0);

	$inspect(channel);

	onMount(async () => {
		Logger.debug('mounted');

		await init();

		Logger.debug(`onMount - ${channel.name}`);
		$channelCache = $channelCache.add(channel.name);

		updateStreamInfo(await getStream(channel.name, client));

		clearInterval(streamRefreshInterval);
		streamRefreshInterval = setInterval(async () => {
			Logger.debug('get stream tick');
			updateStreamInfo(await getStream(channel.name, client));
		}, 60000);

		// Logger.debug(`onMount - loading msg cache for ${channel}`);
		// messages = messageCache.get(channel);

		Logger.debug(`onMount - subscribing to ${channel}`);
		$chatClient.sub(channel.name, twitchMsgHandler);

		if (channel.user) {
			Logger.debug(`onMount - loading badges for ${channel}`);
			loadChannelBadges(channel.user, client.api, GlobalBadgeCache);

			Logger.debug(`onMount - loading emotes for ${channel}`);
			loadChannelEmotes(channel.user, client.api, GlobalEmoteCache);
		}

		autoscrollDebounceFn();
	});

	onDestroy(() => {
		Logger.debug('destroyed');

		Logger.debug(`onDestroy - unsubscribing from ${channel.name}`);
		$chatClient.unsub(channel.name);

		// Logger.debug(`onDestroy - persisting msg cache for ${channel}`);
		// messageCache.set(channel, [...messages]);
		messages = [];

		Logger.debug(`onDestroy - clearing stream refresh interval`);
		clearInterval(streamRefreshInterval);
		updateStreamInfo(null);
	});

	const init = async () => {
		Logger.debug('init');

		const validToken = await client.token.validate();

		let user = await getUserByName(channel.name, client);
		if (!user) {
			Logger.error('failed to get user info', channel.name);
			return;
		}
		channel.user = user;

		if (validToken) {
			$chatClient.token = client.token;
		}
	};

	const updateStreamInfo = (info: HelixStream | null) => {
		Logger.debug('updateStreamInfo: ', info?.title, info?.viewers, info?.startDate);
		if (info) {
			channel.isLive = true;
			channel.streamInfo = {
				title: info.title,
				viewers: info.viewers,
				startDate: info.startDate
			};
		} else {
			channel.isLive = false;
			channel.streamInfo = undefined;
		}
	};

	const uptime = (date: Date): number => {
		return (Date.now() - date.getTime()) / 1000;
	};

	const formatTime = (seconds: number) => {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.round(seconds % 60);
		return [h, m > 9 ? m : h ? '0' + m : m || '0', s > 9 ? s : '0' + s].filter(Boolean).join(':');
	};

	// TODO: write a test for this
	const parseThirdPartyEmotes = (element: ParsedMessageTextPart): BasicParsedMessagePart[] => {
		const TOK_SEP = ' ';

		let newElements = [];

		const tokens = element.text.split(TOK_SEP);
		let newTokens: string[] = [];
		for (let j = 0; j < tokens.length; j++) {
			if (GlobalEmoteCache.HasName(tokens[j])) {
				// all of the new tokens we have so far become element's text
				element.text = newTokens.join(TOK_SEP) + ' ';
				// reset our new set of tokens
				newTokens = [];
				// we update the new length of element
				element.length = element.text.length;
				// push element to newmessageparts
				newElements.push(element);
				// we create an emote part
				const ce = GlobalEmoteCache.GetByName(tokens[j]);
				Logger.trace('global emote', ce);
				let emote: ParsedMessageEmotePart = {
					type: 'emote',
					position: element.position + element.length,
					length: ce.name.length,
					id: ce.id,
					name: ce.name,
					displayInfo: new ChatEmote({ code: ce.name, id: ce.id })
				};
				// push emote part to new message parts
				newElements.push(emote);
				// create a new text element
				element = {
					type: 'text',
					position: emote.position + emote.length,
					length: 0,
					text: ''
				};
				// continue;
			} else {
				newTokens.push(tokens[j]);
			}
		}
		element.text = ' ' + newTokens.join(TOK_SEP);
		element.length = element.text.length;

		newElements.push(element);

		return newElements;
	};

	const twitchMsgHandler = (text: string, msg?: TwitchPrivateMessage) => {
		let m: message;

		// TODO: eventually move to our own parser so we can do a single pass
		let messageParts = parseChatMessage(text, msg?.emoteOffsets ?? new Map<string, string[]>());

		let newMessageParts: BasicParsedMessagePart[] = [];
		messageParts.forEach((element, i) => {
			if (element.type === types.TEXT_TOKEN) {
				Logger.trace(messageParts.length, i, element);
				const newElements = parseThirdPartyEmotes(element);
				newMessageParts.push(...newElements);
			} else {
				newMessageParts.push(element);
			}
		});

		// TODO: fix support for your own messages being sent being properly rendered
		m = {
			id: msg?.id ?? (Math.random() + 1).toString(36).substring(7),
			ts: (msg?.date ?? new Date()).toLocaleTimeString('en', { timeStyle: 'short' }),
			username: msg?.userInfo.displayName ?? currentUser.displayName,
			messageParts: newMessageParts,
			color: msg?.userInfo.color ?? GREY_NAME_COLOR,
			index: nextIndex(),
			raw: msg
		};
		msgHandler(m);
	};

	const shouldScroll = (): boolean => {
		if (autoscrollDebounce) {
			autoscroll = true;
		} else {
			const scrollAmount = div ? div.offsetHeight + div.scrollTop : 0;
			// determine whether we should auto-scroll
			// once the DOM is updated...
			// if the scroll amount matches the tail minus a buffer amount then autoscroll
			autoscroll = div && scrollAmount > div.scrollHeight - AUTOSCROLL_BUFFER;
		}
		return autoscroll;
	};

	const processAutoscroll = async () => {
		await tick();

		if (shouldScroll()) {
			div.scrollTo({ top: div.scrollHeight, left: 0, behavior });
		}
	};

	const msgHandler = async (msg: message) => {
		messages.push(msg);
		if (messages.length > messageLimit) messages.shift();
		await processAutoscroll();
	};

	const submitForm = (event: SubmitEvent) => {
		event.preventDefault();
		let target = event.target as HTMLFormElement;

		if (hasInput) {
			$chatClient
				.say(channel.name, input)
				.then(() => twitchMsgHandler(input))
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
		processAutoscroll();
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
	<div class="flex p-1 pb-2 border-b border-b-base-300">
		<!-- TODO: now that I have real tabs, this should be something else -->
		<div class="flex items-center normal-case text-xl pl-1 pr-2 border-r-2">#{channel.name}</div>

		{#if channel.streamInfo}
			<div class="flex items-center pl-1 ml-2 text-sm">{channel.streamInfo?.title}</div>
		{:else if currentUser.isAnon}
			<div class="flex items-center pl-1 ml-2 text-sm">Unknown</div>
		{/if}
	</div>

	<div class="flex-grow overflow-y-auto overflow-x-hidden neg-horiz-p-2 text-sm" bind:this={div}>
		{#each messages as msg (msg.id)}
			<div
				class="pl-2 pr-2 pt-1 pb-1 inline-block align-middle w-full"
				style={evenOddClass(msg.index)}
			>
				<span class="text-xs text-gray-500 whitespace-nowrap">{msg.ts}</span>
				<Badges message={msg.raw} channel={channel.name} />
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
									src={GlobalEmoteCache.Get(m.id).url}
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
			onclick={pausedChatBtnAction}
			class:hidden={autoscroll}
			class="absolute bottom-24 bg-slate-700 hover:bg-slate-600 p-2 rounded-lg items-center justify-center text-center text-xs"
		>
			Chat Paused - click to scroll
		</button>
	</span>

	<form onsubmit={submitForm}>
		<div class="form-control p-1 border-t border-t-base-300">
			<div class="p-1 text-sm">
				{#if channel.streamInfo}
					{channel.streamInfo.viewers} viewers, {formatTime(uptime(channel.streamInfo.startDate))} uptime
				{/if}
			</div>
			<div class="relative">
				<!-- TODO: properly pad input like the password field so text doesn't go behind button -->
				<!-- TODO: use custom input and input bordered classes to get darker BW outline default, small primary when focused, full outline with text-->
				<input
					bind:value={input}
					bind:this={chatInput}
					disabled={currentUser.isAnon}
					use:keyRedirect
					type="text"
					class="w-full input input-bordered focus:input-primary hover:input-primary"
					style="padding-right: 3rem;"
					placeholder="Chat away..."
					tabindex="0"
				/>
				<!-- svelte-ignore a11y_consider_explicit_label -->
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
