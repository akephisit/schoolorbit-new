<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { getBudgetHistory, type BudgetHistoryPage } from '#lib/api/budget.js';
	import { BudgetRegion } from '#lib/budget/region.svelte.js';
	import { dateLabel } from '#lib/budget/presentation.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { History as HistoryIcon, ChevronDown } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	let {
		kind,
		id,
		revision
	}: { kind: 'projects' | 'requests' | 'adjustments'; id: string; revision: number } = $props();
	const region = new BudgetRegion<BudgetHistoryPage>();
	let opened = $state(false),
		loadedRevision = $state(-1),
		offset = $state(0);
	const labels: Record<string, string> = {
		created: 'สร้างฉบับร่าง',
		saved: 'แก้ไขฉบับร่าง',
		amendment_started: 'เริ่มแก้ไขแผน',
		payment_recorded: 'บันทึกการจ่ายเงิน',
		refund_recorded: 'คืนเงิน',
		closed: 'ปิดคำขอ',
		rejected: 'ไม่อนุมัติ',
		returned: 'ส่งกลับแก้ไข',
		reviewed: 'ตรวจสอบแล้ว',
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
	$effect(() => {
		if (opened && loadedRevision !== revision) {
			loadedRevision = revision;
			untrack(() => load(0));
		}
	});
	onDestroy(() => region.dispose());
</script>

<Card.Root class="gap-0 py-0"
	><details
		class="group"
		ontoggle={(event) => {
			opened = event.currentTarget.open;
		}}
	>
		<summary class="flex cursor-pointer list-none items-center gap-2 p-4 font-semibold sm:p-6"
			><HistoryIcon
				class="size-4 text-muted-foreground"
				aria-hidden="true"
			/>ประวัติการดำเนินการ<ChevronDown
				class="ml-auto size-4 text-muted-foreground transition-transform group-open:rotate-180"
				aria-hidden="true"
			/></summary
		>
		<Card.Content class="space-y-4 pb-4 sm:pb-6" aria-busy={region.loading}>
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
							<p class="font-medium">
								{labels[item.action] ?? 'เปลี่ยนแปลงข้อมูล'} · ฉบับ {item.revision}
							</p>
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
		</Card.Content>
	</details></Card.Root
>
