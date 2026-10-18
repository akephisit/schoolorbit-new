<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		url,
		lastId,
		count,
		limit = 50
	}: { url: string; lastId?: string; count: number; limit?: number } = $props();
	const current = $derived(new URL(url));
	const next = $derived.by(() => {
		const next = new URL(url);
		if (lastId) next.searchParams.set('afterId', lastId);
		return next.pathname + next.search;
	});
	const first = $derived.by(() => {
		const first = new URL(url);
		first.searchParams.delete('afterId');
		return first.pathname + first.search;
	});
</script>

{#if current.searchParams.has('afterId') || count === limit}<nav
		aria-label="แบ่งหน้ารายการ"
		class="flex justify-end gap-2"
	>
		{#if current.searchParams.has('afterId')}<Button href={first} variant="ghost">หน้าแรก</Button
			>{/if}{#if count === limit && lastId}<Button href={next} variant="outline">ถัดไป</Button>{/if}
	</nav>{/if}
