<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import Logger from '$utils/log';

	interface Props {
		emote: Emote;
	}

	let { emote }: Props = $props();
	$inspect(emote);

	let id = $derived(emote.id);
	// TODO: format should use a 'enable motion' setting to pick animated/static
	let format = $derived(emote.format.length > 0 ? emote.format[emote.format.length - 1] : 'static');
	// TODO: theme mode should match a dark/light theme setting
	let theme_mode = $derived(
		emote.theme_mode.length > 0 ? emote.theme_mode[emote.theme_mode.length - 1] : 'dark'
	);
	// TODO: for now just always load the largest one for best quality
	let scale = $derived(emote.scale.length > 0 ? emote.scale[emote.scale.length - 1] : '1.0');

	$effect(() => {
		// https://static-cdn.jtvnw.net/emoticons/v2/{{id}}/{{format}}/{{theme_mode}}/{{scale}}
		Logger.debug(id, format, theme_mode, scale);
	});

	let emote_url = $derived(
		`https://static-cdn.jtvnw.net/emoticons/v2/${id}/${format}/${theme_mode}/${scale}`
	);
	$inspect(emote_url);
</script>

<img class="inline size-6" src={emote_url} alt={emote.name} />
