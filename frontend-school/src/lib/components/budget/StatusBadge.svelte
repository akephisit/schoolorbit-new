<script lang="ts">
	import type { BudgetStatus } from '#lib/api/budget.js';
	import { statusLabels } from '#lib/budget/presentation.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let { status }: { status: BudgetStatus } = $props();
</script>

<Badge
	variant={status === 'approved' ? 'default' : status === 'rejected' ? 'destructive' : 'secondary'}
	>{statusLabels[status]}</Badge
>
