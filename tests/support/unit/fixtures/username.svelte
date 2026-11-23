<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import type { ChatSettings } from '$lib/bindings';
	import { DEFAULT_APP_SETTINGS } from '$lib/settings';
	import Username from '$lib/components/chat/username.svelte';
	let props: ComponentProps<typeof Username> = $props();
	let settings = $state(structuredClone(DEFAULT_APP_SETTINGS.chat));
	export function updateSettings(patch: Partial<ChatSettings>) {
		settings = { ...settings, ...patch };
	}
</script>

<Username {...props} {settings} />
