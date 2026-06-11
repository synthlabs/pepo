<script lang="ts">
	import { Toaster as Sonner, type ToasterProps as SonnerProps } from 'svelte-sonner';

	let restProps: SonnerProps = $props();
</script>

<Sonner
	class="toaster group"
	style="font-family: inherit; --border-radius: var(--radius); --normal-bg: hsl(var(--popover)); --normal-text: hsl(var(--popover-foreground)); --normal-border: hsl(var(--border)); --gray1: hsl(var(--secondary)); --gray2: hsl(var(--accent)); --gray4: hsl(var(--border)); --gray5: hsl(var(--border)); --gray11: hsl(var(--muted-foreground)); --gray12: hsl(var(--foreground));"
	toastOptions={{
		classes: {
			toast: 'group toast',
			actionButton:
				'group-[.toast]:bg-primary! group-[.toast]:text-primary-foreground! group-[.toast]:hover:bg-primary/90!',
			cancelButton: 'group-[.toast]:bg-muted! group-[.toast]:text-muted-foreground!'
		}
	}}
	{...restProps}
/>
