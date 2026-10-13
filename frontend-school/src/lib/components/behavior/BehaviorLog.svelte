<script lang="ts">
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
	<form onsubmit={filter} class="grid gap-3 rounded-md border p-4 sm:grid-cols-3">
		<div>
			<Label for="behavior-log-search">ค้นหานักเรียนหรือรายการ</Label><Input
				id="behavior-log-search"
				bind:value={search}
			/>
		</div>
		<div>
			<Label for="behavior-log-start">เริ่มวันที่</Label><DatePicker
				id="behavior-log-start"
				value={start}
				onValueChange={(v) => {
					if (v) start = v;
				}}
				required
			/>
		</div>
		<div>
			<Label for="behavior-log-end">ถึงวันที่</Label><DatePicker
				id="behavior-log-end"
				value={end}
				onValueChange={(v) => {
					if (v) end = v;
				}}
				required
			/>
		</div>
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
		<div>
			<Label for="behavior-log-batch">รหัสกลุ่มรายการ</Label><Input
				id="behavior-log-batch"
				bind:value={batchId}
			/>
		</div>
		<label class="flex min-h-[44px] items-center gap-2"
			><Checkbox bind:checked={mine} />รายการที่ฉันบันทึก</label
		><LoadingButton
			class="min-h-[44px] min-w-[44px]"
			type="submit"
			loading={pending === 'read'}
			disabled={!!pending || !!frozen}>ค้นหา</LoadingButton
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
			class="min-h-[44px] min-w-[44px]"
			type="button"
			variant="destructive"
			disabled={!!pending || !!frozen}
			onclick={() => {
				confirming = true;
				requestId = '';
			}}>ยกเลิกที่เลือก {selected.length} รายการ</Button
		>{/if}
	<ul class="divide-y rounded-md border" aria-busy={pending === 'read'}>
		{#each page.items as row (row.id)}<li class="space-y-2 p-4">
				<div class="flex flex-wrap items-start justify-between gap-3">
					<div class="flex min-w-0 items-start gap-3">
						{#if canCancel && row.canCancel}<label class="flex min-h-[44px] items-center"
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
						<div>
							<Button
								type="button"
								variant="link"
								class="min-h-[44px] min-w-[44px] h-auto whitespace-normal p-0 text-left"
								onclick={() => onStudent?.(row.studentId)}>{row.studentName}</Button
							>
							<p class="text-xs text-muted-foreground">{row.studentCode ?? '—'} · {row.homeroom}</p>
							<p>
								{row.itemName} ·
								<span class={row.side === 'merit' ? 'text-success' : 'text-destructive'}
									>{sideLabels[row.side]} {row.points}</span
								>
							</p>
						</div>
					</div>
					<Badge variant={row.cancelledAt ? 'secondary' : 'outline'}
						>{row.cancelledAt ? 'ยกเลิกแล้ว' : 'มีผล'}</Badge
					>
				</div>
				<p class="whitespace-pre-wrap break-words text-sm">{row.detail ?? ''}</p>
				<p class="text-xs text-muted-foreground">
					เกิดเหตุ {row.occurredAt ? formatBehaviorTime(row.occurredAt) : row.date} · บันทึก {formatBehaviorTime(
						row.createdAt
					)} · {row.creatorName || (row.createdBy ? 'ผู้บันทึก' : 'ระบบอัตโนมัติ')} · {sourceLabels[
						row.source
					]}
				</p>
				{#if row.cancelledAt}<p class="text-sm text-muted-foreground">
						ยกเลิก: {row.cancellationReason}
					</p>{/if}
				<div class="flex flex-wrap gap-2">
					{#if row.source !== 'manual' && $can.hasAny(PERMISSIONS.ATTENDANCE_READ_ASSIGNED, PERMISSIONS.ATTENDANCE_READ_SCHOOL, PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED, PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL, PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL)}<Button
							class="min-h-[44px] min-w-[44px]"
							href={sourceHref(row)}
							variant="outline">เปิดเช็กชื่อต้นทาง</Button
						>{/if}{#if row.canCancel && canCancel && row.batchId}<Button
							class="min-h-[44px] min-w-[44px]"
							type="button"
							variant="outline"
							disabled={!!pending || !!frozen}
							onclick={() => void selectBatch(row)}>เลือกยกเลิกทั้งกลุ่ม</Button
						>{/if}
				</div>
			</li>{:else}<li class="p-8 text-center text-muted-foreground">
				ไม่มีรายการในช่วงที่เลือก
			</li>{/each}
	</ul>
	<div class="flex items-center justify-between">
		<Button
			class="min-h-[44px] min-w-[44px]"
			type="button"
			variant="outline"
			disabled={!!pending || !applied.offset}
			onclick={() => void load({ ...applied, offset: Math.max(0, (applied.offset ?? 0) - 50) })}
			>ก่อนหน้า</Button
		><span>{page.total} รายการ</span><Button
			class="min-h-[44px] min-w-[44px]"
			type="button"
			variant="outline"
			disabled={!!pending || (applied.offset ?? 0) + 50 >= page.total}
			onclick={() => void load({ ...applied, offset: (applied.offset ?? 0) + 50 })}>ถัดไป</Button
		>
	</div>
	{#if confirming && canCancel}<form
			onsubmit={cancel}
			class="space-y-3 rounded-md border border-destructive/30 p-4"
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
					class="min-h-[44px] min-w-[44px]"
					type="button"
					variant="outline"
					disabled={!!pending || !!frozen}
					onclick={() => (confirming = false)}>กลับ</Button
				><LoadingButton
					class="min-h-[44px] min-w-[44px]"
					type="submit"
					variant="destructive"
					loading={pending === 'cancel'}>ยืนยันยกเลิก</LoadingButton
				>
			</div>
		</form>{/if}
</section>
