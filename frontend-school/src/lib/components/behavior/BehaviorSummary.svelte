<script lang="ts">
	import { ApiClientError } from '#lib/api/client.js';
	import { untrack, onDestroy } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		adjustBehaviorHistory,
		behaviorSummary,
		type BehaviorStudentSummary,
		type BehaviorDailyTotal
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
		term,
		refreshRevision = 0
	}: { initial: BehaviorStudentSummary; term?: string | null; refreshRevision?: number } = $props();
	let summary = $state(untrack(() => initial));
	let editing = $state<BehaviorDailyTotal | null>(null);
	let delta = $state(0);
	let reason = $state('');
	let pending = $state(false);
	let error = $state('');
	let requestId = $state('');
	let disposed = false;
	const request = new LatestRequest();
	let previousRevision = untrack(() => refreshRevision);
	let refreshing = $state(false);
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	async function refresh() {
		const ticket = request.begin();
		refreshing = true;
		error = '';
		try {
			const result = await behaviorSummary(
				summary.studentId,
				{ academicTermId: term ?? undefined },
				{ signal: ticket.signal }
			);
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
		if (!editing || pending) return;
		pending = true;
		error = '';
		try {
			requestId ||= crypto.randomUUID();
			const result = await adjustBehaviorHistory({
				requestId,
				totalId: editing.id,
				rowVersion: editing.rowVersion,
				delta,
				reason
			});
			if (disposed) return;
			const difference = result.points - editing.points;
			summary.history = {
				...summary.history,
				[editing.side]: summary.history[editing.side] + difference,
				net: summary.history.net + (editing.side === 'merit' ? difference : -difference)
			};
			summary.lifetime = {
				...summary.lifetime,
				[editing.side]: summary.lifetime[editing.side] + difference,
				net: summary.lifetime.net + (editing.side === 'merit' ? difference : -difference)
			};
			summary.historyTotals = summary.historyTotals.map((row) =>
				row.id === result.totalId
					? { ...row, points: result.points, rowVersion: result.rowVersion }
					: row
			);
			editing = null;
			requestId = '';
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'ปรับยอดไม่ได้';
			if (e instanceof ApiClientError && e.status < 500) requestId = '';
		} finally {
			if (!disposed) pending = false;
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
	<div class="grid gap-4 md:grid-cols-3">
		{#each [{ label: 'รอบปัจจุบัน', totals: summary.current }, { label: 'ประวัติที่เก็บไว้', totals: summary.history }, { label: 'สะสมทั้งหมด', totals: summary.lifetime }] as group (group.label)}<Card.Root
				><Card.Header><Card.Title>{group.label}</Card.Title></Card.Header><Card.Content
					><dl class="grid grid-cols-2 gap-2">
						<dt>ความดี</dt>
						<dd class="text-end font-semibold text-success">{group.totals.merit}</dd>
						<dt>ถูกหัก</dt>
						<dd class="text-end font-semibold text-destructive">{group.totals.deduction}</dd>
						<dt>สุทธิ</dt>
						<dd class="text-end font-semibold">{group.totals.net ?? 'ต่างช่วงเวลา'}</dd>
					</dl></Card.Content
				></Card.Root
			>{/each}
	</div>
	<p class="text-sm text-muted-foreground">
		ความดีเริ่ม {summary.meritCycle.startsOn} · คะแนนหักเริ่ม {summary.deductionCycle.startsOn}
	</p>
	<p>
		สุทธิในช่วงเดียวกัน {summary.comparisonStart}–{summary.comparisonEnd}:
		<strong>{summary.comparableCurrent.net ?? 'เปรียบเทียบไม่ได้'}</strong
		>{#if !summary.comparisonExact}<span class="block text-sm text-muted-foreground"
				>มีการรีเซ็ตระหว่างวัน จึงยังไม่มีผลสุทธิที่เปรียบเทียบตรงช่วงได้</span
			>{/if}
	</p>
	<details>
		<summary class="min-h-11 cursor-pointer py-2"
			>ประวัติยอดรายวัน ({summary.historyTotals.length})</summary
		>
		<ul class="divide-y">
			{#each summary.historyTotals as row (row.id)}<li
					class="flex flex-wrap items-center justify-between gap-2 py-3"
				>
					<span
						>{row.date} · {sideLabels[row.side]} · {sourceLabels[row.source]} · {row.points} คะแนน</span
					>{#if $can.has(PERMISSIONS.BEHAVIOR_CORRECT_SCHOOL)}<Button
							type="button"
							variant="outline"
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
			class="space-y-3 rounded-md border p-4"
			onsubmit={adjust}
		>
			<h3 class="font-semibold">ปรับยอด {editing.date} · {sideLabels[editing.side]}</h3>
			{#if error}<PageState
					variant="error"
					title="ปรับยอดไม่สำเร็จ"
					description={error}
				/>{/if}<Label for="behavior-adjust-delta">จำนวนที่ปรับ (+ เพิ่ม / − ลด)</Label><Input
				id="behavior-adjust-delta"
				type="number"
				bind:value={delta}
				required
			/><Label for="behavior-adjust-reason">เหตุผล *</Label><Textarea
				id="behavior-adjust-reason"
				bind:value={reason}
				required
				maxlength={1000}
			/>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="outline" disabled={pending} onclick={() => (editing = null)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending}>บันทึกการปรับยอด</LoadingButton>
			</div>
		</form>{/if}
</section>
