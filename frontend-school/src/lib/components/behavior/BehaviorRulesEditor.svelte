<script lang="ts">
	import { formatBehaviorDate } from '#lib/features/behavior/presentation.js';
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
	import * as Table from '#lib/components/ui/table/index.js';
	import BehaviorFormSection from './BehaviorFormSection.svelte';
	import { CalendarCheck } from '@lucide/svelte';
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

<BehaviorFormSection
	title="เกณฑ์จากเช็กชื่อ"
	description="แยกมาโรงเรียน รายคาบ และรอบพิเศษ เลือกเกณฑ์กลางหรือเฉพาะระดับชั้นได้"
	icon={CalendarCheck}
>
	<p class="text-sm text-muted-foreground">
		ยังไม่ได้เช็คไม่คิดคะแนนระหว่างวัน หลังเที่ยงคืนของวันที่นับยอด
		ระบบจะบันทึกเป็นขาดและใช้เกณฑ์คะแนนของสถานะขาด
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
			<div class="space-y-2">
				<Label for="rule-points">จำนวนคะแนน</Label><Input
					id="rule-points"
					type="number"
					min="0"
					bind:value={draft.points}
					disabled={draft.action === 'none'}
					required
				/>
			</div>
			<div class="space-y-2">
				<Label for="rule-effective">วันเริ่มใช้เกณฑ์</Label><DatePicker
					id="rule-effective"
					value={draft.effectiveFrom}
					onValueChange={(v) => {
						if (v) draft.effectiveFrom = v;
					}}
				/>
			</div>
		</fieldset>
		<div class="flex justify-end border-t pt-4">
			<LoadingButton type="submit" loading={pending}>บันทึกเกณฑ์</LoadingButton>
		</div>
	</form>
	<div class="overflow-hidden rounded-xl border">
		<Table.Root
			><Table.Header
				><Table.Row
					><Table.Head>ประเภท / สถานะ</Table.Head><Table.Head>ระดับชั้น</Table.Head><Table.Head
						>การคิดคะแนน</Table.Head
					><Table.Head>เริ่มใช้</Table.Head><Table.Head></Table.Head></Table.Row
				></Table.Header
			><Table.Body
				>{#each rules as rule (rule.id)}<Table.Row
						><Table.Cell class="whitespace-normal"
							><span class="block font-medium">{sourceLabels[rule.kind]}</span><span
								class="text-xs text-muted-foreground"
								>{resultLabels[
									writableAttendanceResults.find((status) => status === rule.status) ?? 'unchecked'
								]}</span
							></Table.Cell
						><Table.Cell class="whitespace-normal"
							>{initial.options.gradeLevels.find((option) => option.id === rule.gradeLevelId)
								?.label ?? 'เกณฑ์กลาง'}</Table.Cell
						><Table.Cell class="whitespace-normal"
							><span
								class={rule.action === 'merit'
									? 'text-success'
									: rule.action === 'deduction'
										? 'text-destructive'
										: 'text-muted-foreground'}>{actionLabels[rule.action]} {rule.points}</span
							></Table.Cell
						><Table.Cell class="whitespace-normal"
							>{formatBehaviorDate(rule.effectiveFrom)}</Table.Cell
						><Table.Cell class="text-end"
							><Button
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
							></Table.Cell
						></Table.Row
					>{:else}<Table.Row
						><Table.Cell colspan={5} class="p-6 text-center text-muted-foreground"
							>ยังไม่มีรายการ</Table.Cell
						></Table.Row
					>{/each}</Table.Body
			></Table.Root
		>
	</div>
</BehaviorFormSection>
