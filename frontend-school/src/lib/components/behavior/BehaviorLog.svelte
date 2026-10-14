<script lang="ts">
	import { formatBehaviorDate } from '#lib/features/behavior/presentation.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { onDestroy, untrack } from 'svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		behaviorEntries,
		cancelBehaviorEntries,
		type BehaviorReportPage,
		type BehaviorReportQuery,
		type CancelBehaviorEntries,
		type BehaviorReportEntry
	} from '#lib/api/behavior.js';
	import {
		sides,
		sources,
		sideLabels,
		sourceLabels,
		formatBehaviorTime
	} from '#lib/features/behavior/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import { History, Search } from '@lucide/svelte';
	let {
		initial,
		query,
		onStudent,
		onChanged
	}: {
		initial: BehaviorReportPage;
		query: BehaviorReportQuery;
		onStudent?: (id: string) => void;
		onChanged?: () => void;
	} = $props();
	let page = $state(untrack(() => initial));
	let applied = $state<BehaviorReportQuery>(untrack(() => ({ ...query, limit: 50, offset: 0 })));
	let search = $state('');
	let start = $state(untrack(() => query.start));
	let end = $state(untrack(() => query.end));
	let side = $state('');
	let source = $state('');
	let mine = $state(false);
	let cancelled = $state('');
	let batchId = $state('');
	let selected = $state<string[]>([]);
	let reason = $state('');
	let confirming = $state(false);
	let pending = $state('');
	let error = $state('');
	let requestId = $state('');
	let frozen = $state.raw<CancelBehaviorEntries | null>(null);
	const request = new LatestRequest();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	const canCancel = $derived(
		$can.hasAny(PERMISSIONS.BEHAVIOR_DELETE_OWN, PERMISSIONS.BEHAVIOR_DELETE_SCHOOL)
	);
	async function load(filters: BehaviorReportQuery) {
		if (disposed || frozen || (pending && pending !== 'read')) return;
		const token = request.begin();
		pending = 'read';
		error = '';
		try {
			const result = await behaviorEntries(filters, { signal: token.signal });
			if (request.isCurrent(token.revision)) {
				page = result;
				applied = filters;
				selected = [];
			}
		} catch (e) {
			if (request.isCurrent(token.revision))
				error = e instanceof Error ? e.message : 'โหลดรายงานไม่ได้';
		} finally {
			if (request.isCurrent(token.revision)) pending = '';
		}
	}
	function filter(e: SubmitEvent) {
		e.preventDefault();
		if (pending || frozen) return;
		void load({
			...query,
			start,
			end,
			search: search.trim() || undefined,
			side: sides.find((s) => s === side),
			source: sources.find((s) => s === source),
			mine: mine || undefined,
			cancelled: cancelled === '' ? undefined : cancelled === 'yes',
			batchId: batchId || undefined,
			offset: 0,
			limit: 50
		});
	}
	async function selectBatch(row: BehaviorReportEntry) {
		if (!row.batchId || pending || frozen || disposed) return;
		const ticket = request.begin();
		pending = 'batch';
		error = '';
		try {
			const result = await behaviorEntries(
				{
					...query,
					start: row.date,
					end: row.date,
					batchId: row.batchId,
					offset: 0,
					limit: 10000
				},
				{ signal: ticket.signal }
			);
			if (result.items.length !== result.total)
				throw new Error('กลุ่มมีรายการเกินขนาดที่เลือกได้ กรุณายกเลิกเป็นชุดย่อย');
			if (disposed || !request.isCurrent(ticket.revision)) return;
			selected = result.items.filter((r) => r.canCancel).map((r) => r.id);
			confirming = true;
		} catch (e) {
			if (request.isCurrent(ticket.revision) && !disposed)
				error = e instanceof Error ? e.message : 'โหลดกลุ่มไม่ได้';
		} finally {
			if (request.isCurrent(ticket.revision) && !disposed) pending = '';
		}
	}
	async function cancel(e: SubmitEvent) {
		e.preventDefault();
		if (pending || (!selected.length && !frozen)) return;
		pending = 'cancel';
		error = '';
		try {
			requestId ||= crypto.randomUUID();
			frozen ??= { requestId, entryIds: [...selected], reason };
			const payload = frozen;
			await cancelBehaviorEntries(payload);
			if (disposed) return;
			const ids = new Set(payload.entryIds);
			page.items = page.items.map((row) =>
				ids.has(row.id)
					? {
							...row,
							cancelledAt: new Date().toISOString(),
							cancellationReason: payload.reason,
							canCancel: false
						}
					: row
			);
			selected = [];
			confirming = false;
			reason = '';
			requestId = '';
			frozen = null;
			onChanged?.();
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'ยกเลิกไม่ได้';
			if (e instanceof ApiClientError && e.status < 500) {
				requestId = '';
				frozen = null;
			}
		} finally {
			if (!disposed) pending = '';
		}
	}
	function sourceHref(row: BehaviorReportEntry) {
		const p = new SvelteURLSearchParams({ academicTermId: row.academicTermId, date: row.date });
		if (row.sourceSessionId) p.set('sessionId', row.sourceSessionId);
		return row.sourceSessionId
			? `/staff/attendance/sessions/${encodeURIComponent(row.sourceSessionId)}?${p}`
			: `/staff/attendance?${p}`;
	}
</script>

<section class="space-y-4" aria-label="ประวัติรายการคะแนน">
	<div class="flex items-center gap-3">
		<span class="flex size-9 items-center justify-center rounded-xl bg-primary/10 text-primary"
			><History class="size-4" aria-hidden="true" /></span
		>
		<div>
			<h2 class="text-lg font-semibold">ประวัติรายการคะแนน</h2>
			<p class="text-sm text-muted-foreground">รายการเพิ่มความดีและหักคะแนนตามช่วงที่เลือก</p>
		</div>
	</div>
	<form onsubmit={filter} class="grid gap-3 rounded-xl border bg-card p-3 sm:grid-cols-3 sm:p-4">
		<div class="space-y-2">
			<Label for="behavior-log-search">ค้นหานักเรียนหรือรายการ</Label><Input
				id="behavior-log-search"
				bind:value={search}
			/>
		</div>
		<div class="space-y-2">
			<Label for="behavior-log-start">เริ่มวันที่</Label><DatePicker
				id="behavior-log-start"
				value={start}
				onValueChange={(v) => {
					if (v) start = v;
				}}
				required
			/>
		</div>
		<div class="space-y-2">
			<Label for="behavior-log-end">ถึงวันที่</Label><DatePicker
				id="behavior-log-end"
				value={end}
				onValueChange={(v) => {
					if (v) end = v;
				}}
				required
			/>
		</div>
		<details class="sm:col-span-3">
			<summary class="cursor-pointer py-3 text-sm font-medium">ตัวกรองเพิ่มเติม</summary>
			<div class="grid gap-3 border-t pt-3 sm:grid-cols-3">
				<BehaviorSelect
					id="behavior-log-side"
					label="ฝั่งคะแนน"
					value={side}
					options={[
						{ value: '', label: 'ทุกฝั่ง' },
						...sides.map((s) => ({ value: s, label: sideLabels[s] }))
					]}
					onValueChange={(v) => (side = v)}
				/><BehaviorSelect
					id="behavior-log-source"
					label="ที่มา"
					value={source}
					options={[
						{ value: '', label: 'ทุกที่มา' },
						...sources.map((s) => ({ value: s, label: sourceLabels[s] }))
					]}
					onValueChange={(v) => (source = v)}
				/><BehaviorSelect
					id="behavior-log-cancelled"
					label="สถานะรายการ"
					value={cancelled}
					options={[
						{ value: '', label: 'ทุกสถานะ' },
						{ value: 'no', label: 'มีผล' },
						{ value: 'yes', label: 'ยกเลิกแล้ว' }
					]}
					onValueChange={(v) => (cancelled = v)}
				/>
				<div class="space-y-2">
					<Label for="behavior-log-batch">รหัสกลุ่มรายการ</Label><Input
						id="behavior-log-batch"
						bind:value={batchId}
					/>
				</div>
				{#if onStudent}<label class="flex items-center gap-2"
						><Checkbox bind:checked={mine} />รายการที่ฉันบันทึก</label
					>
				{/if}
			</div>
		</details>
		<LoadingButton type="submit" loading={pending === 'read'} disabled={!!pending || !!frozen}
			><Search class="size-4" aria-hidden="true" />ค้นหา</LoadingButton
		>
	</form>
	{#if error}<PageState
			variant="error"
			title="ดำเนินการไม่สำเร็จ"
			description={error}
			actionLabel="โหลดรายงานใหม่"
			onaction={() => void load(applied)}
		/>{/if}
	{#if canCancel && selected.length}<Button
			type="button"
			variant="destructive"
			disabled={!!pending || !!frozen}
			onclick={() => {
				confirming = true;
				requestId = '';
			}}>ยกเลิกที่เลือก {selected.length} รายการ</Button
		>{/if}
	<div class="overflow-hidden rounded-xl border bg-card" aria-busy={pending === 'read'}>
		<Table.Root class="block lg:table" aria-label="รายการคะแนน">
			<Table.Header class="hidden bg-muted/30 lg:table-header-group"
				><Table.Row
					><Table.Head class="px-4">นักเรียน</Table.Head><Table.Head>รายการและคะแนน</Table.Head
					><Table.Head>ที่มา / ผู้บันทึก</Table.Head><Table.Head class="px-4 text-end"
						>จัดการ</Table.Head
					></Table.Row
				></Table.Header
			>
			<Table.Body class="block lg:table-row-group">
				{#each page.items as row (row.id)}
					<Table.Row class="block space-y-2 p-4 lg:table-row lg:space-y-0 lg:p-0">
						<Table.Cell class="block whitespace-normal p-0 lg:table-cell lg:px-4 lg:py-3"
							><div class="flex min-w-0 items-start gap-3">
								{#if canCancel && row.canCancel}<label class="flex items-center justify-center"
										><Checkbox
											aria-label={`เลือกยกเลิก ${row.studentName} ${row.itemName}`}
											disabled={!!requestId}
											checked={selected.includes(row.id)}
											onCheckedChange={(v) =>
												(selected = v
													? [...new Set([...selected, row.id])]
													: selected.filter((id) => id !== row.id))}
										/></label
									>{/if}
								<div class="min-w-0">
									{#if onStudent}<Button
											type="button"
											variant="link"
											class="h-auto whitespace-normal p-0 text-left font-semibold"
											onclick={() => onStudent?.(row.studentId)}>{row.studentName}</Button
										>{:else}<p class="font-semibold">{row.studentName}</p>{/if}
									<p class="text-xs text-muted-foreground">
										{row.studentCode ?? '—'} · {row.homeroom}
									</p>
								</div>
							</div></Table.Cell
						>
						<Table.Cell class="block whitespace-normal p-0 lg:table-cell lg:py-3"
							><p class="font-medium">{row.itemName}</p>
							<div class="mt-2 flex flex-wrap items-center gap-2">
								<span
									class={row.side === 'merit'
										? 'font-semibold text-success'
										: 'font-semibold text-destructive'}>{sideLabels[row.side]} {row.points}</span
								><Badge variant={row.cancelledAt ? 'secondary' : 'outline'}
									>{row.cancelledAt ? 'ยกเลิกแล้ว' : 'มีผล'}</Badge
								>
							</div>
							<p class="mt-2 text-xs text-muted-foreground">
								เกิดเหตุ {row.occurredAt
									? formatBehaviorTime(row.occurredAt)
									: formatBehaviorDate(row.date)}
							</p>
							{#if row.detail || row.cancelledAt}<details class="mt-2">
									<summary class="cursor-pointer py-2 text-sm text-muted-foreground"
										>รายละเอียดเพิ่มเติม</summary
									>{#if row.detail}<p class="whitespace-pre-wrap break-words pb-2 text-sm">
											{row.detail}
										</p>{/if}{#if row.cancelledAt}<p class="pb-2 text-sm text-muted-foreground">
											ยกเลิก: {row.cancellationReason}
										</p>{/if}
								</details>{/if}</Table.Cell
						>
						<Table.Cell class="block whitespace-normal p-0 lg:table-cell lg:py-3"
							><p class="text-sm">{sourceLabels[row.source]}</p>
							<p class="mt-1 text-xs text-muted-foreground">
								{row.creatorName || (row.createdBy ? 'ผู้บันทึก' : 'ระบบอัตโนมัติ')}
							</p>
							<p class="mt-1 text-xs text-muted-foreground">
								บันทึก {formatBehaviorTime(row.createdAt)}
							</p></Table.Cell
						>
						<Table.Cell class="block whitespace-normal p-0 lg:table-cell lg:px-4 lg:py-3"
							><div class="flex flex-wrap gap-2 lg:justify-end">
								{#if row.source !== 'manual' && $can.hasAny(PERMISSIONS.ATTENDANCE_READ_ASSIGNED, PERMISSIONS.ATTENDANCE_READ_SCHOOL, PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED, PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL, PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL)}<Button
										href={sourceHref(row)}
										variant="outline">เปิดเช็กชื่อต้นทาง</Button
									>{/if}{#if row.canCancel && canCancel && row.batchId}<Button
										type="button"
										variant="outline"
										disabled={!!pending || !!frozen}
										onclick={() => void selectBatch(row)}>เลือกยกเลิกทั้งกลุ่ม</Button
									>{/if}
							</div></Table.Cell
						>
					</Table.Row>
				{:else}<Table.Row class="block lg:table-row"
						><Table.Cell
							colspan={4}
							class="block p-8 text-center whitespace-normal text-muted-foreground lg:table-cell"
							>ไม่มีรายการในช่วงที่เลือก</Table.Cell
						></Table.Row
					>{/each}
			</Table.Body>
		</Table.Root>
	</div>
	<div class="flex items-center justify-between gap-3 text-sm">
		<Button
			type="button"
			variant="outline"
			disabled={!!pending || !applied.offset}
			onclick={() => void load({ ...applied, offset: Math.max(0, (applied.offset ?? 0) - 50) })}
			>ก่อนหน้า</Button
		><span>{page.total} รายการ</span><Button
			type="button"
			variant="outline"
			disabled={!!pending || (applied.offset ?? 0) + 50 >= page.total}
			onclick={() => void load({ ...applied, offset: (applied.offset ?? 0) + 50 })}>ถัดไป</Button
		>
	</div>
	{#if confirming && canCancel}<form
			onsubmit={cancel}
			class="space-y-4 rounded-xl border border-destructive/30 bg-card p-4 sm:p-5"
		>
			<h3 class="font-semibold">ยกเลิกคะแนน {selected.length} รายการ</h3>
			<p class="text-sm">ประวัติยังอยู่ แต่คะแนนจะไม่นับในยอด</p>
			<Label for="behavior-cancel-reason">เหตุผล *</Label><Textarea
				id="behavior-cancel-reason"
				bind:value={reason}
				disabled={!!pending || !!requestId}
				required
				maxlength={1000}
			/>
			<div class="flex justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					disabled={!!pending || !!frozen}
					onclick={() => (confirming = false)}>กลับ</Button
				><LoadingButton type="submit" variant="destructive" loading={pending === 'cancel'}
					>ยืนยันยกเลิก</LoadingButton
				>
			</div>
		</form>{/if}
</section>
