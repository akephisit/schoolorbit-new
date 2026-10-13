<script lang="ts">
	import { untrack, onDestroy } from 'svelte';
	import {
		saveBehaviorRule,
		type BehaviorAttendanceRule,
		type BehaviorSelectorOptions,
		type SaveBehaviorAttendanceRule
	} from '#lib/api/behavior.js';
	import { resultLabels, writableAttendanceResults } from '#lib/api/attendance.js';
	import {
		sources,
		sourceLabels,
		actions,
		actionLabels,
		bangkokDateTime
	} from '#lib/features/behavior/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	let {
		initial
	}: { initial: { rules: BehaviorAttendanceRule[]; options: BehaviorSelectorOptions } } = $props();
	let rules = $state(untrack(() => initial.rules));
	let draft = $state<SaveBehaviorAttendanceRule>({
		kind: 'arrival',
		status: 'present',
		gradeLevelId: null,
		effectiveFrom: bangkokDateTime().date,
		action: 'none',
		points: 0
	});
	let pending = $state(false);
	let disposed = false;
	onDestroy(() => (disposed = true));
	let error = $state('');
	async function save(e: SubmitEvent) {
		e.preventDefault();
		pending = true;
		error = '';
		try {
			const result = await saveBehaviorRule({
				...draft,
				points: draft.action === 'none' ? 0 : draft.points
			});
			if (disposed) return;
			rules = [...rules.filter((r) => r.id !== result.id), result];
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'บันทึกไม่ได้';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<section class="space-y-4 rounded-md border p-4">
	<h2 class="text-lg font-semibold">เกณฑ์จากเช็กชื่อ</h2>
	<p class="text-sm text-muted-foreground">
		แยกมาโรงเรียน รายคาบ และรอบพิเศษ เลือกเกณฑ์กลางหรือเฉพาะระดับชั้นได้
	</p>
	<form onsubmit={save} class="space-y-3">
		{#if error}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={error} />{/if}
		<fieldset disabled={pending} class="grid gap-3 sm:grid-cols-3">
			<BehaviorSelect
				id="rule-kind"
				label="ประเภทเช็กชื่อ"
				value={draft.kind}
				options={sources
					.filter((s) => s !== 'manual')
					.map((s) => ({ value: s, label: sourceLabels[s] }))}
				onValueChange={(v) => {
					const s = sources.find((s) => s === v && s !== 'manual');
					if (s) draft.kind = s;
				}}
			/><BehaviorSelect
				id="rule-status"
				label="สถานะเช็กชื่อ"
				value={draft.status}
				options={writableAttendanceResults.map((s) => ({ value: s, label: resultLabels[s] }))}
				onValueChange={(v) => (draft.status = v)}
			/><BehaviorSelect
				id="rule-grade"
				label="ระดับชั้นของเกณฑ์"
				value={draft.gradeLevelId ?? ''}
				options={[
					{ value: '', label: 'เกณฑ์กลางทุกระดับ' },
					...initial.options.gradeLevels.map((o) => ({ value: o.id, label: o.label }))
				]}
				onValueChange={(v) => (draft.gradeLevelId = v || null)}
			/><BehaviorSelect
				id="rule-action"
				label="การคิดคะแนน"
				value={draft.action}
				options={actions.map((s) => ({ value: s, label: actionLabels[s] }))}
				onValueChange={(v) => {
					const a = actions.find((a) => a === v);
					if (a) {
						draft.action = a;
						if (a === 'none') draft.points = 0;
					}
				}}
			/>
			<div>
				<Label for="rule-points">จำนวนคะแนน</Label><Input
					id="rule-points"
					type="number"
					min="0"
					bind:value={draft.points}
					disabled={draft.action === 'none'}
					required
				/>
			</div>
			<div>
				<Label for="rule-effective">วันเริ่มใช้เกณฑ์</Label><DatePicker
					id="rule-effective"
					value={draft.effectiveFrom}
					onValueChange={(v) => {
						if (v) draft.effectiveFrom = v;
					}}
				/>
			</div>
		</fieldset>
		<div class="flex justify-end">
			<LoadingButton type="submit" loading={pending}>บันทึกเกณฑ์</LoadingButton>
		</div>
	</form>
	<ul class="divide-y">
		{#each rules as rule (rule.id)}<li class="py-3 text-sm">
				{sourceLabels[rule.kind]} · {resultLabels[
					writableAttendanceResults.find((s) => s === rule.status) ?? 'unchecked'
				]} · {initial.options.gradeLevels.find((o) => o.id === rule.gradeLevelId)?.label ??
					'เกณฑ์กลาง'} · {actionLabels[rule.action]}
				{rule.points} · เริ่ม {rule.effectiveFrom}<Button
					type="button"
					variant="ghost"
					onclick={() =>
						(draft = {
							kind: rule.kind,
							status: rule.status,
							gradeLevelId: rule.gradeLevelId,
							effectiveFrom: bangkokDateTime().date,
							action: rule.action,
							points: rule.points
						})}>ตั้งเกณฑ์ใหม่</Button
				>
			</li>{/each}
	</ul>
</section>
