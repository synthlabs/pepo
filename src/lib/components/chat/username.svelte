<script lang="ts">
	import type { ChatSettings } from '$lib/bindings';
	import { DEFAULT_APP_SETTINGS } from '$lib/settings';
	import { formatUsername, usernameColor, type UsernameActivation } from '$lib/chat/username';
	import { userDirectory, type UserDirectory } from '$lib/chat/users.svelte';
	import { cn } from '$lib/utils';

	let {
		userId,
		login,
		displayName,
		color,
		kind,
		originalText,
		channelId,
		channelLogin,
		settings = DEFAULT_APP_SETTINGS.chat,
		directory = userDirectory,
		onactivate
	}: {
		userId?: string | null;
		login: string;
		displayName?: string | null;
		color?: string | null;
		kind: 'author' | 'mention' | 'system';
		originalText?: string;
		channelId: string;
		channelLogin: string;
		settings?: ChatSettings;
		directory?: UserDirectory;
		onactivate?: (target: UsernameActivation) => void;
	} = $props();
	let cached = $derived(kind === 'author' ? undefined : directory.get(userId, login));
	let resolvedName = $derived(cached?.display_name || displayName || login);
	let resolvedLogin = $derived(cached?.login || login);
	let name = $derived(
		formatUsername(resolvedName, resolvedLogin, settings.show_login_for_different_display_name)
	);
	let label = $derived(
		kind === 'mention'
			? settings.use_display_names_in_mentions
				? `@${name}`
				: (originalText ?? `@${login}`)
			: name
	);
	let renderedColor = $derived(usernameColor(kind === 'author' ? color : (cached?.color ?? color)));
</script>

<button
	type="button"
	class={cn(
		'focus-visible:outline-foreground inline cursor-pointer appearance-none border-0 bg-transparent p-0 text-left align-baseline leading-[inherit] text-inherit underline-offset-2 select-text hover:underline focus-visible:rounded-sm focus-visible:underline focus-visible:outline-2 focus-visible:outline-offset-2',
		(kind !== 'mention' || settings.bold_mentions) && 'font-bold'
	)}
	style:color={renderedColor}
	data-chat-username={kind}
	data-user-id={userId || cached?.user_id}
	data-user-login={resolvedLogin}
	data-system-user-id={kind === 'system' ? userId : undefined}
	onclick={(event) =>
		onactivate?.({
			userId: userId || cached?.user_id || null,
			login: resolvedLogin,
			displayName: resolvedName,
			channelId,
			channelLogin,
			anchor: event.currentTarget
		})}>{label}</button
>
