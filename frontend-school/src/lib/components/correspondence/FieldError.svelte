<script lang="ts">
	let { id, message }: { id: string; message?: string } = $props();
</script>

{#if message}<p {id} class="text-sm text-destructive">{message}</p>{/if}
