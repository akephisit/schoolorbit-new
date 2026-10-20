<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getBudgetHistory, type BudgetHistoryPage } from '#lib/api/budget.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { dateLabel } from '#lib/budget/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	let { kind, id }: { kind: 'projects' | 'requests' | 'adjustments'; id: string } = $props();
	const region = new BudgetRegion<BudgetHistoryPage>();
	let opened = false,
		offset = $state(0);
	const labels: Record<string, string> = {
		created: 'สร้างฉบับร่าง',
		updated: 'แก้ไขฉบับร่าง',
		submitted: 'ส่งพิจารณา',
		approved: 'อนุมัติ',
		review: 'ตรวจสอบ',
		approve: 'อนุมัติ',
		reject: 'ไม่อนุมัติ',
		return_for_revision: 'ส่งกลับแก้ไข',
		amendment: 'เริ่มแก้ไขแผน',
		payment: 'บันทึกการจ่ายเงิน',
		refund: 'คืนเงิน',
		close: 'ปิดคำขอ'
	};
	function load(next = offset) {
		offset = next;
		region.retry((signal) => getBudgetHistory(kind, id, { signal, query: { limit: 20, offset } }));
	}
	onDestroy(() => region.dispose());
</script>

<details
	class="rounded-xl border bg-card p-3 sm:p-4"
	ontoggle={(event) => {
		if (event.currentTarget.open && !opened) {
			opened = true;
			load();
		}
	}}
>
	<summary class="cursor-pointer font-semibold">ประวัติการดำเนินการ</summary>
	<div class="mt-3 space-y-3" aria-busy={region.loading}>
		{#if region.error}<PageState
				variant="error"
				title="โหลดประวัติไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => load()}
			/>{/if}{#if region.loading && !region.value}<TableSkeleton
				columns={2}
				rows={3}
			/>{:else if region.value}<ol class="divide-y">
				{#each region.value.items as item (item.id)}<li class="py-2 text-sm">
						<p>{labels[item.action] ?? 'เปลี่ยนแปลงข้อมูล'} · ฉบับ {item.revision}</p>
						<p class="text-muted-foreground">
							{dateLabel(item.createdAt)}{item.comment ? ` · ${item.comment}` : ''}
						</p>
					</li>{/each}
			</ol>
			<div class="flex justify-between">
				<Button
					size="sm"
					variant="outline"
					disabled={offset === 0 || region.loading}
					onclick={() => load(Math.max(0, offset - 20))}>ก่อนหน้า</Button
				><Button
					size="sm"
					variant="outline"
					disabled={offset + 20 >= region.value.total || region.loading}
					onclick={() => load(offset + 20)}>ถัดไป</Button
				>
			</div>{/if}
	</div>
</details>
