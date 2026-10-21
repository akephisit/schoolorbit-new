<script lang="ts">
	import type { BudgetStatus } from '#lib/api/budget.js';
	import { statusLabels } from '#lib/budget/presentation.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let { status }: { status: BudgetStatus } = $props();
</script>

<Badge
	variant={status === 'rejected' ? 'destructive' : 'outline'}
	class={status === 'approved'
		? 'border-success/20 bg-success/10 text-success'
		: status === 'returned'
			? 'border-warning/20 bg-warning/10 text-warning'
			: status === 'rejected'
				? ''
				: 'text-muted-foreground'}>{statusLabels[status]}</Badge
>
