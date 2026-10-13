<script lang="ts">
	import {
		writableAttendanceResults,
		resultLabels,
		type AttendanceScoreRule
	} from '#lib/api/attendance.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import AttendanceSelect from './AttendanceSelect.svelte';
	let { value = $bindable(null) }: { value?: AttendanceScoreRule[] | null } = $props();
	const actions: AttendanceScoreRule['action'][] = ['none', 'merit', 'deduction'];
	const labels = { none: 'ไม่คิดคะแนน', merit: 'เพิ่มความดี', deduction: 'หักคะแนน' };
	function enable(enabled: boolean) {
		value = enabled
			? writableAttendanceResults.map((result) => ({ result, action: 'none', points: 0 }))
			: null;
	}
</script>

<section class="space-y-3 rounded-md border p-4" aria-label="คะแนนรอบพิเศษ">
	<label class="flex min-h-11 items-center gap-2"
		><Checkbox checked={value !== null} onCheckedChange={enable} />กำหนดคะแนนเฉพาะรอบนี้</label
	>{#if value}<p class="text-sm text-muted-foreground">
			เลือกไม่คิดคะแนน เพิ่มความดี หรือหักคะแนนให้แต่ละสถานะ
		</p>
		{#each value as rule (rule.result)}<div
				class="grid grid-cols-[5rem_minmax(0,1fr)_5rem] items-center gap-2"
			>
				<Label for={`special-score-${rule.result}`}>{resultLabels[rule.result]}</Label
				><AttendanceSelect
					id={`special-score-${rule.result}`}
					label={`คะแนนเมื่อ${resultLabels[rule.result]}`}
					value={rule.action}
					options={actions.map((a) => ({ value: a, label: labels[a] }))}
					onValueChange={(v) => {
						const action = actions.find((a) => a === v);
						if (action) {
							rule.action = action;
							if (action === 'none') rule.points = 0;
						}
					}}
				/><Input
					aria-label={`จำนวนคะแนนเมื่อ${resultLabels[rule.result]}`}
					type="number"
					min="0"
					bind:value={rule.points}
					disabled={rule.action === 'none'}
					required
				/>
			</div>{/each}{:else}<p class="text-sm text-muted-foreground">
			ใช้เกณฑ์รอบพิเศษจากระบบพฤติกรรม
		</p>{/if}
</section>
