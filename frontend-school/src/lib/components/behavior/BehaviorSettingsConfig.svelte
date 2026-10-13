<script lang="ts">
	import { untrack, onDestroy } from 'svelte';
	import { saveBehaviorSettings, type BehaviorSettings } from '#lib/api/behavior.js';
	import { schedules, scheduleLabels } from '#lib/features/behavior/presentation.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	let { initial }: { initial: BehaviorSettings } = $props();
	let draft = $state(untrack(() => ({ ...initial })));
	let meritCap = $state(untrack(() => initial.dailyMeritCap ?? ''));
	let deductionCap = $state(untrack(() => initial.dailyDeductionCap ?? ''));
	let pending = $state(false);
	let disposed = false;
	onDestroy(() => (disposed = true));
	let error = $state('');
	let saved = $state(false);
	async function save(e: SubmitEvent) {
		e.preventDefault();
		pending = true;
		error = '';
		saved = false;
		try {
			draft = await saveBehaviorSettings({
				...draft,
				dailyMeritCap: meritCap === '' ? null : Number(meritCap),
				dailyDeductionCap: deductionCap === '' ? null : Number(deductionCap)
			});
			if (disposed) return;
			saved = true;
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'บันทึกไม่ได้';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form class="space-y-4 rounded-md border p-4" onsubmit={save}>
	<h2 class="text-lg font-semibold">การเชื่อมคะแนนและรอบรีเซ็ต</h2>
	{#if error}<PageState
			variant="error"
			title="บันทึกไม่สำเร็จ"
			description={error}
		/>{/if}{#if saved}<p role="status">บันทึกการตั้งค่าแล้ว</p>{/if}
	<fieldset disabled={pending} class="space-y-4">
		{#each [{ key: 'enabled', label: 'เปิดการคิดคะแนนจากเช็กชื่อ' }, { key: 'countAbsentLessons', label: 'นับคะแนนขาดคาบซ้ำเมื่อขาดโรงเรียนทั้งวัน' }, { key: 'countSpecial', label: 'นับคะแนนรอบพิเศษด้วย' }] as option (option.key)}<label
				class="flex min-h-11 items-center gap-2"
				><Checkbox
					checked={option.key === 'enabled'
						? draft.enabled
						: option.key === 'countAbsentLessons'
							? draft.countAbsentLessons
							: draft.countSpecial}
					onCheckedChange={(v) => {
						if (option.key === 'enabled') draft.enabled = v;
						else if (option.key === 'countAbsentLessons') draft.countAbsentLessons = v;
						else draft.countSpecial = v;
					}}
				/>{option.label}</label
			>{/each}
		<div class="grid gap-4 sm:grid-cols-2">
			<div>
				<Label for="merit-cap">เพดานความดีอัตโนมัติต่อวัน (เว้นว่าง = ไม่จำกัด)</Label><Input
					id="merit-cap"
					type="number"
					min="0"
					bind:value={meritCap}
				/>
			</div>
			<div>
				<Label for="deduction-cap">เพดานคะแนนหักอัตโนมัติต่อวัน</Label><Input
					id="deduction-cap"
					type="number"
					min="0"
					bind:value={deductionCap}
				/>
			</div>
			<div>
				<Label for="merit-reset">รีเซ็ตคะแนนความดี</Label><BehaviorSelect
					id="merit-reset"
					label="รีเซ็ตความดี"
					value={draft.meritReset}
					options={schedules.map((s) => ({ value: s, label: scheduleLabels[s] }))}
					onValueChange={(v) => {
						const s = schedules.find((s) => s === v);
						if (s) draft.meritReset = s;
					}}
				/>
			</div>
			<div>
				<Label for="deduction-reset">รีเซ็ตคะแนนที่ถูกหัก</Label><BehaviorSelect
					id="deduction-reset"
					label="รีเซ็ตคะแนนหัก"
					value={draft.deductionReset}
					options={schedules.map((s) => ({ value: s, label: scheduleLabels[s] }))}
					onValueChange={(v) => {
						const s = schedules.find((s) => s === v);
						if (s) draft.deductionReset = s;
					}}
				/>
			</div>
		</div>
	</fieldset>
	<div class="flex justify-end">
		<LoadingButton type="submit" loading={pending}>บันทึกการตั้งค่า</LoadingButton>
	</div>
</form>
