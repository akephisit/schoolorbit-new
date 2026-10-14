<script lang="ts">
	let { id, message }: { id: string; message?: string | null } = $props();
</script>

{#if message}<p {id} class="text-sm text-destructive" aria-live="polite">{message}</p>{/if}
