<script lang="ts">
	import { LoaderCircle } from 'lucide-svelte';
	import { Button, type ButtonProps } from '$lib/components/ui/button';

	let {
		loading = false,
		loadingLabel = 'กำลังดำเนินการ...',
		disabled,
		children,
		...restProps
	}: ButtonProps & {
		loading?: boolean;
		loadingLabel?: string;
	} = $props();
</script>

<Button disabled={disabled || loading} {...restProps}>
	{#if loading}
		<LoaderCircle class="h-4 w-4 animate-spin" />
		{loadingLabel}
	{:else}
		{@render children?.()}
	{/if}
</Button>
