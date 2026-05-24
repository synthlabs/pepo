<script lang="ts">
    import { commands } from "./bindings";

    let message = $state("");
    let pending = $state(false);

    async function ping() {
        pending = true;
        try {
            message = await commands.internalPing();
        } catch (error) {
            message = error instanceof Error ? error.message : String(error);
        } finally {
            pending = false;
        }
    }
</script>

<div class="bg-background/95 fixed right-3 bottom-3 z-50 flex items-center gap-2 rounded-md border p-2 text-xs shadow">
    <button class="rounded border px-2 py-1 disabled:opacity-50" disabled={pending} onclick={ping}>
        Ping
    </button>
    {#if message}
        <span>{message}</span>
    {/if}
</div>
