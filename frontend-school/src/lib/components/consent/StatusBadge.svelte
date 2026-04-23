<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { SvelteComponent } from 'svelte';

	interface Props {
		variant: 'default' | 'secondary' | 'destructive' | 'outline';
		// Lucide v1 uses SvelteComponentTyped (Svelte 4 class components)
		// eslint-disable-next-line @typescript-eslint/no-explicit-any -- SvelteComponentTyped subclasses require 'any' params for Svelte 4/5 interop
		icon: typeof SvelteComponent<any>;
		label: string;
	}

	let { variant, icon: Icon, label }: Props = $props();
</script>

<Badge {variant}>
	<Icon class="h-3 w-3 mr-1" />
	{label}
</Badge>
