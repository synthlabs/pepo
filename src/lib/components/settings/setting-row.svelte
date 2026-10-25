<script lang="ts">
	import { Switch } from 'bits-ui';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		controlValue,
		controlUpdate,
		validateControlInput,
		PROVIDER_LABELS,
		SETTINGS_CATEGORIES,
		type SettingControl
	} from '$lib/settings/controls';
	import {
		getNormalizedAppSettings,
		settingsState,
		updateAppSettings
	} from '$lib/stores/settings.svelte';

	let { control, showCategory = false }: { control: SettingControl; showCategory?: boolean } =
		$props();
	let settings = $derived(getNormalizedAppSettings());
	let value = $derived(control.kind === 'provider-order' ? '' : controlValue(settings, control));
	let id = $derived(`setting-${control.id.replaceAll('.', '-')}`);
	let input = $state('');
	let editing = $state(false);
	let error = $state('');

	$effect(() => {
		const current = String(value);
		if (!editing) {
			input = current;
			error = '';
		}
	});

	function update(next: string | number | boolean) {
		void updateAppSettings(controlUpdate(control, next));
	}

	function commit() {
		if (!editing || !settingsState.ready) return;
		const parsed = validateControlInput(control, input);
		if (parsed.error !== undefined) {
			error = parsed.error;
			return;
		}
		editing = false;
		error = '';
		input = String(parsed.value);
		if (parsed.value !== value) update(parsed.value);
	}

	function moveProvider(index: number, direction: number) {
		const id = settings.emotes.providers[index].id;
		void updateAppSettings((current) => {
			const providers = current.emotes.providers.map((provider) => ({ ...provider }));
			const position = providers.findIndex((provider) => provider.id === id);
			const next = position + direction;
			if (position < 0 || next < 0 || next >= providers.length) return {};
			[providers[position], providers[next]] = [providers[next], providers[position]];
			return { emotes: { providers } };
		});
	}
</script>

<div
	class="bg-background flex min-w-0 flex-col gap-3 px-4 py-4 @min-[480px]/settings:flex-row @min-[480px]/settings:items-center @min-[480px]/settings:justify-between @min-[480px]/settings:gap-6"
	data-setting-row={control.id}
>
	<div class="min-w-0 flex-1">
		{#if showCategory}<p class="text-muted-foreground mb-1 text-xs">
				{SETTINGS_CATEGORIES.find((category) => category.id === control.category)?.label}
			</p>{/if}
		<div id={`${id}-label`} class="text-sm font-medium">{control.label}</div>
		<p id={`${id}-description`} class="text-muted-foreground mt-1 text-[13px] leading-relaxed">
			{control.description}
		</p>
	</div>
	<div class="shrink-0 @min-[480px]/settings:max-w-[55%]">
		{#if control.kind === 'toggle' || control.kind === 'provider'}
			<Switch.Root
				{id}
				checked={Boolean(value)}
				onCheckedChange={update}
				disabled={!settingsState.ready}
				aria-labelledby={`${id}-label`}
				aria-describedby={`${id}-description`}
				class="bg-input data-[state=checked]:bg-primary focus-visible:ring-ring inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
			>
				<Switch.Thumb
					class="pointer-events-none block size-4 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-4"
				/>
			</Switch.Root>
		{:else if control.kind === 'theme'}
			<div
				class="bg-muted inline-flex gap-1 rounded-md border p-1"
				role="group"
				aria-labelledby={`${id}-label`}
			>
				{#each control.options ?? [] as option (option.value)}
					<Button
						variant={value === option.value ? 'outline' : 'ghost'}
						size="sm"
						class="h-7 px-2.5 text-xs"
						aria-pressed={value === option.value}
						disabled={!settingsState.ready}
						onclick={() => update(option.value)}>{option.label}</Button
					>
				{/each}
			</div>
		{:else if control.kind === 'select'}
			<select
				{id}
				value={String(value)}
				onchange={(event) => update(event.currentTarget.value)}
				disabled={!settingsState.ready}
				aria-labelledby={`${id}-label`}
				aria-describedby={`${id}-description`}
				class="border-input bg-background focus-visible:ring-ring h-8 max-w-full rounded-md border px-2 text-sm outline-none focus-visible:ring-2 disabled:opacity-50"
			>
				{#each control.options ?? [] as option (option.value)}<option value={option.value}
						>{option.label}</option
					>{/each}
			</select>
		{:else if control.kind === 'provider-order'}
			<ol
				aria-labelledby={`${id}-label`}
				aria-describedby={`${id}-description`}
				class="flex flex-col gap-1"
			>
				{#each settings.emotes.providers as provider, index (provider.id)}
					<li class="flex items-center gap-1" data-provider-priority={provider.id}>
						<span class="text-muted-foreground w-4 text-xs tabular-nums">{index + 1}</span>
						<span class="min-w-16 flex-1 text-sm">{PROVIDER_LABELS[provider.id]}</span>
						<Button
							variant="ghost"
							size="icon"
							class="size-7"
							aria-label={`Move ${PROVIDER_LABELS[provider.id]} up`}
							disabled={!settingsState.ready || index === 0}
							onclick={() => moveProvider(index, -1)}><ArrowUp /></Button
						>
						<Button
							variant="ghost"
							size="icon"
							class="size-7"
							aria-label={`Move ${PROVIDER_LABELS[provider.id]} down`}
							disabled={!settingsState.ready || index === settings.emotes.providers.length - 1}
							onclick={() => moveProvider(index, 1)}><ArrowDown /></Button
						>
					</li>
				{/each}
			</ol>
		{:else}
			<div class="flex items-center gap-2">
				<Input
					{id}
					type={control.kind === 'number' ? 'number' : 'text'}
					value={input}
					min={control.kind === 'number' ? 1 : undefined}
					max={control.kind === 'number' ? (control.max ?? Number.MAX_SAFE_INTEGER) : undefined}
					step={control.kind === 'number' ? 1 : undefined}
					class="h-8 w-28 text-sm"
					disabled={!settingsState.ready}
					aria-labelledby={`${id}-label`}
					aria-describedby={`${id}-description${error ? ` ${id}-error` : ''}`}
					aria-invalid={error ? true : undefined}
					oninput={(event) => {
						input = event.currentTarget.value;
						editing = true;
						error = '';
					}}
					onblur={commit}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							commit();
						}
					}}
				/>
				{#if control.unit}<span class="text-muted-foreground text-xs">{control.unit}</span>{/if}
			</div>
			{#if error}<p id={`${id}-error`} class="text-destructive mt-1 max-w-56 text-xs" role="alert">
					{error}
				</p>{/if}
		{/if}
	</div>
</div>
