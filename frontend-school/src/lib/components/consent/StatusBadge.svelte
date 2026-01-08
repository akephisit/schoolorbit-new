<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	
	interface Props {
		variant: 'default' | 'secondary' | 'destructive' | 'outline';
		icon: any; // Lucide icon component
		label: string;
	}
	
	let { variant, icon: Icon, label }: Props = $props();
</script>

<Badge {variant}>
	<Icon class="h-3 w-3 mr-1" />
	{label}
</Badge>
