<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { destination } from '#lib/budget/presentation.js';
	let {
		path,
		query,
		total,
		limit,
		offset
	}: {
		path: string;
		query: Record<string, string | number | undefined>;
		total: number;
		limit: number;
		offset: number;
	} = $props();
</script>

<div class="flex flex-wrap items-center justify-between gap-3">
	<p class="text-sm text-muted-foreground">
		{total ? `${offset + 1}–${Math.min(offset + limit, total)} จาก ${total} รายการ` : '0 รายการ'}
	</p>
	<div class="flex gap-2">
		<Button
			size="sm"
			variant="outline"
			disabled={offset <= 0}
			href={offset > 0
				? destination(path, { ...query, offset: Math.max(0, offset - limit) })
				: undefined}>ก่อนหน้า</Button
		><Button
			size="sm"
			variant="outline"
			disabled={offset + limit >= total}
			href={offset + limit < total
				? destination(path, { ...query, offset: offset + limit })
				: undefined}>ถัดไป</Button
		>
	</div>
</div>
