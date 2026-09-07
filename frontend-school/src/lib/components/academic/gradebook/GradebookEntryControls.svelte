<script lang="ts">
	import type { GradebookControl, GradebookPhaseCode } from '$lib/api/academicGradebook';
	import type {
		LearnerEvaluationControl,
		LearnerEvaluationDomain
	} from '$lib/api/academicLearnerEvaluations';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import { Label } from '$lib/components/ui/label';
	import { Switch } from '$lib/components/ui/switch';
	import { ClipboardCheck, ShieldCheck } from 'lucide-svelte';

	let {
		gradebookControls,
		evaluationControls,
		canManageGradebook,
		canManageEvaluation,
		busyKey = '',
		ontoggleGradebook,
		ontoggleEvaluation
	}: {
		gradebookControls: GradebookControl[];
		evaluationControls: LearnerEvaluationControl[];
		canManageGradebook: boolean;
		canManageEvaluation: boolean;
		busyKey?: string;
		ontoggleGradebook: (control: GradebookControl) => void;
		ontoggleEvaluation: (control: LearnerEvaluationControl) => void;
	} = $props();

	const phaseLabels: Record<GradebookPhaseCode, string> = {
		before_midterm: 'ก่อนกลางภาค',
		midterm: 'กลางภาค',
		after_midterm: 'หลังกลางภาค',
		final: 'ปลายภาค'
	};
	const domainLabels: Record<LearnerEvaluationDomain, string> = {
		desirable_characteristic: 'คุณลักษณะอันพึงประสงค์',
		reading_thinking_writing: 'การอ่าน คิดวิเคราะห์ และเขียน'
	};
</script>

<Card.Root class="gap-0 py-0">
	<Card.Header class="border-b px-4 py-3">
		<div class="flex items-center justify-between gap-3">
			<div>
				<Card.Title class="flex items-center gap-2 text-base">
					<ShieldCheck class="size-4 text-primary" /> ช่วงเวลาที่ครูกรอกได้
				</Card.Title>
				<Card.Description>ผู้ดูแลยังแก้ไขได้เมื่อปิดช่วงสำหรับครูผู้สอน</Card.Description>
			</div>
			<Badge variant="outline">ตั้งค่าทั้งภาคเรียน</Badge>
		</div>
	</Card.Header>
	<Card.Content class="grid gap-0 p-0 lg:grid-cols-2">
		{#if canManageGradebook}
			<div class="border-b lg:border-r lg:border-b-0">
				<div class="flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5 text-sm font-medium">
					<ClipboardCheck class="size-4" /> คะแนนรายวิชา
				</div>
				<div class="divide-y">
					{#each gradebookControls as control (control.id)}
						<div class="flex items-center justify-between gap-3 px-4 py-3">
							<Label for={`gradebook-control-${control.id}`}
								>{phaseLabels[control.phaseCode as GradebookPhaseCode] ?? control.phaseCode}</Label
							>
							<Switch
								id={`gradebook-control-${control.id}`}
								checked={control.scoreEntryEnabled}
								disabled={Boolean(busyKey)}
								onclick={() => ontoggleGradebook(control)}
							/>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		{#if canManageEvaluation}
			<div>
				<div class="flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5 text-sm font-medium">
					<ClipboardCheck class="size-4" /> การประเมินผู้เรียน
				</div>
				<div class="divide-y">
					{#each evaluationControls as control (control.id)}
						<div class="flex items-center justify-between gap-3 px-4 py-3">
							<Label for={`evaluation-control-${control.id}`}>{domainLabels[control.domain]}</Label>
							<Switch
								id={`evaluation-control-${control.id}`}
								checked={control.entryEnabled}
								disabled={Boolean(busyKey)}
								onclick={() => ontoggleEvaluation(control)}
							/>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</Card.Content>
</Card.Root>
