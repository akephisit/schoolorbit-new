<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { resultLabels, type AttendanceResult } from '#lib/api/attendance.js';
	let { value }: { value: AttendanceResult | null } = $props();
	const result = $derived(value ?? 'unchecked');
</script>

<Badge
	variant="outline"
	class={result === 'present'
		? 'border-success/30 bg-success/10 text-success'
		: result === 'absent' || result === 'skipped'
			? 'border-destructive/30 bg-destructive/10 text-destructive'
			: result === 'late'
				? 'border-warning/30 bg-warning/10 text-warning'
				: 'text-muted-foreground'}>{resultLabels[result]}</Badge
>
