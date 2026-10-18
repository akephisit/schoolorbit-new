<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import {
		getLeaveUsage,
		type LeaveUsageReport,
		type LeaveUsageQuery
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { leaveAmount, leaveDate } from '#lib/staff-leave/presentation.js';
	import { leaveUsageCsv } from '#lib/staff-leave/report.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import Choice from './Choice.svelte';
	let {
		source,
		query
	}: { source: Promise<RouteLoadResult<LeaveUsageReport | null>>; query: LeaveUsageQuery } =
		$props();
	const region = new LeaveRegion<LeaveUsageReport | null>();
	let from = $state<string | undefined>(untrack(() => query.from)),
		to = $state<string | undefined>(untrack(() => query.to)),
		kind = $state(untrack(() => query.kind ?? 'all')),
		error = $state('');
	const allowed = $derived(
		$can.hasAny(PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN, PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL)
	);
	$effect.pre(() => region.consume(source));
	onDestroy(() => region.dispose());
	function filter(event: SubmitEvent) {
		event.preventDefault();
		error = '';
		if (!from || !to || from > to) {
			error = 'เลือกช่วงวันที่ให้ถูกต้อง';
			return;
		}
		const search = new SvelteURLSearchParams({ from, to });
		if (kind !== 'all') search.set('kind', kind);
		void goto(`/staff/leave/reports?${search}`);
	}
	function download() {
		if (!region.value || !allowed) return;
		const blob = new Blob([leaveUsageCsv(region.value)], { type: 'text/csv;charset=utf-8' });
		const url = URL.createObjectURL(blob),
			anchor = document.createElement('a');
		anchor.href = url;
		anchor.download = `สรุปการลา-${query.from}-${query.to}.csv`;
		anchor.click();
		URL.revokeObjectURL(url);
	}
</script>

<PageShell
	contentClass="space-y-4"
	title="สรุปการใช้สิทธิ์"
	description="คำขอที่เริ่มในช่วงวันที่เลือก · แสดงจำนวนทั้งคำขอที่อนุมัติและรอพิจารณา"
	backHref="/staff/leave"
>
	{#snippet actions()}{#if allowed}<Button
				variant="outline"
				disabled={!region.value}
				onclick={download}>ดาวน์โหลด CSV</Button
			>{/if}{/snippet}
	<form class="border-b pb-3" onsubmit={filter}>
		<div
			class="grid grid-cols-2 items-end gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_auto]"
		>
			<div class="space-y-2">
				<Label for="usage-from">ตั้งแต่วันที่</Label><DatePicker
					id="usage-from"
					ariaLabel="รายงานตั้งแต่วันที่"
					bind:value={from}
				/>
			</div>
			<div class="space-y-2">
				<Label for="usage-to">ถึงวันที่</Label><DatePicker
					id="usage-to"
					ariaLabel="รายงานถึงวันที่"
					bind:value={to}
				/>
			</div>
			<Choice
				id="usage-kind"
				label="กลุ่มคำขอ"
				bind:value={kind}
				options={[
					{ value: 'all', label: 'ทั้งหมด' },
					{ value: 'leave', label: 'การลา' },
					{ value: 'exit', label: 'ออกนอกโรงเรียน' }
				]}
			/><Button type="submit">แสดงรายงาน</Button>
		</div>
		{#if error}<p role="alert" class="mt-2 text-sm text-destructive">{error}</p>{/if}
	</form>
	<section data-testid="leave-usage" aria-busy={region.loading} class="space-y-4">
		<p class="text-sm text-muted-foreground">
			{leaveDate(query.from)} – {leaveDate(query.to)} · {$can.has(
				PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL
			)
				? 'ข้อมูลตามสิทธิ์ทั้งโรงเรียน'
				: 'ข้อมูลของคุณ'}
		</p>
		{#if region.error}<PageState
				variant="error"
				title="โหลดสรุปการใช้สิทธิ์ไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => getLeaveUsage({ signal, query }))}
			/>{/if}
		{#if region.loading && !region.value}<div role="status" aria-label="กำลังโหลดสรุปการใช้สิทธิ์">
				<TableSkeleton columns={6} />
			</div>{:else if region.value?.items.length}<div
				class="hidden overflow-hidden rounded-lg border bg-card md:block"
			>
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>บุคลากร</Table.Head><Table.Head>ประเภทคำขอ</Table.Head><Table.Head
								>รอบปีเริ่ม</Table.Head
							><Table.Head>อนุมัติแล้ว</Table.Head><Table.Head>รอพิจารณา</Table.Head><Table.Head
								>สิทธิที่กำหนด</Table.Head
							></Table.Row
						></Table.Header
					><Table.Body
						>{#each region.value.items as item (`${item.userId}|${item.typeId}|${item.quotaYearStart}|${item.policyVersionId}`)}<Table.Row
								><Table.Cell class="font-medium">{item.displayName}</Table.Cell><Table.Cell
									>{item.typeName}</Table.Cell
								><Table.Cell>{leaveDate(item.quotaYearStart)}</Table.Cell><Table.Cell
									>{leaveAmount(item.approvedAmount, item.unit)}</Table.Cell
								><Table.Cell>{leaveAmount(item.reservedAmount, item.unit)}</Table.Cell><Table.Cell
									>{item.annualLimit == null
										? 'ไม่กำหนด'
										: leaveAmount(item.annualLimit, item.unit)}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
			<ul
				class="divide-y rounded-lg border bg-card md:hidden"
				aria-label="สรุปการใช้สิทธิ์รายบุคลากร"
			>
				{#each region.value.items as item (`${item.userId}|${item.typeId}|${item.quotaYearStart}|${item.policyVersionId}`)}<li
						class="space-y-2 p-3"
					>
						<div>
							<p class="break-words font-medium">{item.displayName}</p>
							<p class="text-sm">{item.typeName} · รอบปี {leaveDate(item.quotaYearStart)}</p>
						</div>
						<dl class="grid grid-cols-3 gap-2 text-sm">
							<div>
								<dt class="text-xs text-muted-foreground">อนุมัติแล้ว</dt>
								<dd class="mt-1 font-medium">{leaveAmount(item.approvedAmount, item.unit)}</dd>
							</div>
							<div>
								<dt class="text-xs text-muted-foreground">รอพิจารณา</dt>
								<dd class="mt-1 font-medium">{leaveAmount(item.reservedAmount, item.unit)}</dd>
							</div>
							<div>
								<dt class="text-xs text-muted-foreground">สิทธิที่กำหนด</dt>
								<dd class="mt-1">
									{item.annualLimit == null ? 'ไม่กำหนด' : leaveAmount(item.annualLimit, item.unit)}
								</dd>
							</div>
						</dl>
					</li>{/each}
			</ul>{:else if !region.error}<PageState
				title="ยังไม่มีการใช้สิทธิ์ในช่วงที่เลือก"
				description="เลือกช่วงวันที่อื่นเพื่อดูรายการ"
			/>{/if}
	</section>
</PageShell>
