<script lang="ts">
	import { formatBehaviorDate } from '#lib/features/behavior/presentation.js';
	import { Award, ShieldAlert, Scale, History } from '@lucide/svelte';
	import { ApiClientError } from '#lib/api/client.js';
	import { untrack, onDestroy } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		adjustBehaviorHistory,
		behaviorSummary,
		type BehaviorStudentSummary,
		type BehaviorDailyTotal,
		type AdjustBehaviorHistory
	} from '#lib/api/behavior.js';
	import { sideLabels, sourceLabels } from '#lib/features/behavior/presentation.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	let {
		initial,
		refreshRevision = 0
	}: { initial: BehaviorStudentSummary; refreshRevision?: number } = $props();
	let summary = $state(untrack(() => initial));
	let editing = $state<BehaviorDailyTotal | null>(null);
	let delta = $state(0);
	let reason = $state('');
	let pending = $state(false);
	let error = $state('');
	let requestId = $state('');
	let frozen = $state.raw<{ row: BehaviorDailyTotal; payload: AdjustBehaviorHistory } | null>(null);
	let disposed = false;
	const request = new LatestRequest();
	let previousRevision = untrack(() => refreshRevision);
	let refreshing = $state(false);
	let refreshDeferred = false;
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	async function refresh() {
		if (disposed) return;
		if (pending || frozen) {
			refreshDeferred = true;
			return;
		}
		refreshDeferred = false;
		const ticket = request.begin();
		refreshing = true;
		error = '';
		try {
			const result = await behaviorSummary(summary.studentId, {}, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) summary = result;
		} catch (e) {
			if (request.isCurrent(ticket.revision))
				error = e instanceof Error ? e.message : 'โหลดคะแนนไม่ได้';
		} finally {
			if (request.isCurrent(ticket.revision)) refreshing = false;
		}
	}
	$effect(() => {
		if (refreshRevision !== previousRevision) {
			previousRevision = refreshRevision;
			untrack(() => void refresh());
		}
	});

	async function adjust(e: SubmitEvent) {
		e.preventDefault();
		if (!editing || pending || refreshing) return;
		request.abort();
		pending = true;
		error = '';
		try {
			requestId ||= crypto.randomUUID();
			frozen ??= {
				row: { ...editing },
				payload: {
					requestId,
					totalId: editing.id,
					rowVersion: editing.rowVersion,
					delta,
					reason
				}
			};
			const row = frozen.row;
			const result = await adjustBehaviorHistory(frozen.payload);
			if (disposed) return;
			const difference = result.points - row.points;
			summary.history = {
				...summary.history,
				[row.side]: summary.history[row.side] + difference,
				net:
					summary.history.net === null
						? summary.history.net
						: summary.history.net + (row.side === 'merit' ? difference : -difference)
			};
			summary.lifetime = {
				...summary.lifetime,
				[row.side]: summary.lifetime[row.side] + difference,
				net: summary.lifetime.net + (row.side === 'merit' ? difference : -difference)
			};
			summary.historyTotals = summary.historyTotals.map((row) =>
				row.id === result.totalId
					? { ...row, points: result.points, rowVersion: result.rowVersion }
					: row
			);
			editing = null;
			requestId = '';
			frozen = null;
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'ปรับยอดไม่ได้';
			if (e instanceof ApiClientError && e.status < 500) {
				requestId = '';
				frozen = null;
			}
		} finally {
			if (!disposed) {
				pending = false;
				if (refreshDeferred && !frozen) void refresh();
			}
		}
	}
</script>

<section aria-label="สรุปคะแนน" class="space-y-4" aria-busy={refreshing}>
	{#if error && !editing}<PageState
			variant="error"
			title="โหลดคะแนนล่าสุดไม่ได้"
			description={error}
			actionLabel="ลองโหลดใหม่"
			onaction={() => void refresh()}
		/>{/if}
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div>
			<h2 class="text-lg font-semibold">คะแนนของนักเรียน</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				ยอดปัจจุบันแยกจากประวัติและคะแนนสะสมทุกปีการศึกษา
			</p>
		</div>
	</div>
	<section aria-label="รอบปัจจุบัน" class="grid grid-cols-3 gap-3">
		{#each [{ label: 'ความดี', points: summary.current.merit, icon: Award, color: 'text-success', tint: 'bg-success/10' }, { label: 'ถูกหัก', points: summary.current.deduction, icon: ShieldAlert, color: 'text-destructive', tint: 'bg-destructive/10' }, { label: 'สุทธิ', points: summary.current.net, icon: Scale, color: 'text-primary', tint: 'bg-primary/10' }] as metric (metric.label)}
			<Card.Root class="gap-0 py-0"
				><Card.Content class="p-3 sm:p-5">
					<div class="flex items-center justify-between gap-3">
						<p class="text-sm text-muted-foreground">{metric.label}</p>
						<span
							class={`hidden size-9 items-center justify-center rounded-xl sm:flex ${metric.tint} ${metric.color}`}
							><metric.icon class="size-4" aria-hidden="true" /></span
						>
					</div>
					<p
						class={`mt-3 break-words font-semibold tabular-nums ${metric.color} ${metric.points === null ? 'text-lg' : 'text-2xl sm:text-3xl'}`}
					>
						{metric.points ?? 'ต่างช่วงเวลา'}
					</p>
					<p class="mt-1 text-xs text-muted-foreground">
						{metric.points === null ? 'ดูผลเปรียบเทียบช่วงเดียวกันด้านล่าง' : 'คะแนน'}
					</p>
				</Card.Content></Card.Root
			>
		{/each}
	</section>
	<div class="grid gap-4 lg:grid-cols-2">
		{#each [{ label: 'ประวัติรวมทุกภาคเรียน', totals: summary.history, showNet: false, description: 'ยอดที่เก็บไว้จากรอบก่อนและการย้ายเป็นประวัติ' }, { label: 'สะสมทั้งหมด', totals: summary.lifetime, showNet: true, description: 'รวมคะแนนปัจจุบันและประวัติทุกปีการศึกษา' }] as group (group.label)}
			<Card.Root aria-label={group.label} class="gap-0 py-0">
				<Card.Header class="border-b p-4 sm:p-5"
					><div class="flex items-center gap-2">
						<History class="size-4 text-muted-foreground" aria-hidden="true" />
						<h3 class="font-semibold">{group.label}</h3>
					</div>
					<Card.Description>{group.description}</Card.Description></Card.Header
				>
				<Card.Content class="p-4 sm:p-5"
					><dl class="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
						<dt class="text-muted-foreground">ความดี</dt>
						<dd class="text-end font-semibold tabular-nums text-success">{group.totals.merit}</dd>
						<dt class="text-muted-foreground">ถูกหัก</dt>
						<dd class="text-end font-semibold tabular-nums text-destructive">
							{group.totals.deduction}
						</dd>
						{#if group.showNet}<dt class="text-muted-foreground">สุทธิ</dt>
							<dd class="text-end font-semibold tabular-nums">
								{group.totals.net ?? 'ต่างช่วงเวลา'}
							</dd>{/if}
					</dl></Card.Content
				>
			</Card.Root>
		{/each}
	</div>
	<p class="text-sm text-muted-foreground">
		ความดีเริ่ม {formatBehaviorDate(summary.meritCycle.startsOn)} · คะแนนหักเริ่ม {formatBehaviorDate(
			summary.deductionCycle.startsOn
		)}
	</p>
	<p class="rounded-xl border border-primary/20 bg-primary/5 p-4 text-sm">
		สุทธิในช่วงเดียวกัน {formatBehaviorDate(summary.comparisonStart)}–{formatBehaviorDate(
			summary.comparisonEnd
		)}:
		<strong>{summary.comparableCurrent.net ?? 'เปรียบเทียบไม่ได้'}</strong
		>{#if !summary.comparisonExact}<span class="block text-sm text-muted-foreground"
				>รอบคะแนนหรือข้อมูลที่เก็บเป็นประวัติมีช่วงไม่ตรงกัน จึงยังเปรียบเทียบผลสุทธิในช่วงนี้ไม่ได้</span
			>{/if}
	</p>
	<details class="rounded-xl border bg-card px-4 sm:px-5">
		<summary class="[@media(any-pointer:coarse)]:min-h-[44px] cursor-pointer py-2"
			>ประวัติยอดรายวัน ({summary.historyTotals.length})</summary
		>
		<ul class="divide-y">
			{#each summary.historyTotals as row (row.id)}<li
					class="flex flex-wrap items-center justify-between gap-2 py-3"
				>
					<span
						>{formatBehaviorDate(row.date)} · {sideLabels[row.side]} · {sourceLabels[row.source]} · {row.points}
						คะแนน</span
					>{#if $can.has(PERMISSIONS.BEHAVIOR_CORRECT_SCHOOL)}<Button
							type="button"
							variant="outline"
							disabled={pending || !!requestId || refreshing}
							onclick={() => {
								editing = row;
								delta = 0;
								reason = '';
								requestId = '';
							}}>ปรับยอด</Button
						>{/if}
				</li>{/each}
		</ul>
	</details>
	{#if editing && $can.has(PERMISSIONS.BEHAVIOR_CORRECT_SCHOOL)}<form
			class="max-w-3xl space-y-4 rounded-xl border bg-card p-4 sm:p-5"
			onsubmit={adjust}
		>
			<h3 class="font-semibold">
				ปรับยอด {formatBehaviorDate(editing.date)} · {sideLabels[editing.side]}
			</h3>
			{#if error}<PageState
					variant="error"
					title="ปรับยอดไม่สำเร็จ"
					description={error}
				/>{/if}<Label for="behavior-adjust-delta">จำนวนที่ปรับ (+ เพิ่ม / − ลด)</Label><Input
				id="behavior-adjust-delta"
				disabled={pending || !!requestId}
				type="number"
				bind:value={delta}
				required
			/><Label for="behavior-adjust-reason">เหตุผล *</Label><Textarea
				id="behavior-adjust-reason"
				disabled={pending || !!requestId}
				bind:value={reason}
				required
				maxlength={1000}
			/>
			<div class="flex justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					disabled={pending || !!requestId}
					onclick={() => (editing = null)}>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending} disabled={refreshing}
					>บันทึกการปรับยอด</LoadingButton
				>
			</div>
		</form>{/if}
</section>
