<script lang="ts">
	import type { GradebookControl, GradebookPhaseCode } from '$lib/api/academicGradebook';
	import type {
		LearnerEvaluationControl,
		LearnerEvaluationDomain
	} from '$lib/api/academicLearnerEvaluations';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
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
	let open = $state(false);

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

<Button variant="outline" size="sm" onclick={() => (open = true)}>
	<ShieldCheck class="size-4" /> ตั้งค่าการกรอก
</Button>
<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<div class="flex flex-wrap items-center justify-between gap-3 pr-6">
				<div>
					<Dialog.Title class="flex items-center gap-2 text-base">
						<ShieldCheck class="size-4 text-primary" /> ช่วงเวลาที่ครูกรอกได้
					</Dialog.Title>
					<Dialog.Description>ผู้ดูแลยังแก้ไขได้เมื่อปิดช่วงสำหรับครูผู้สอน</Dialog.Description>
				</div>
				<Badge variant="outline">ตั้งค่าทั้งภาคเรียน</Badge>
			</div>
		</Dialog.Header>
		<div
			class={[
				'grid overflow-hidden rounded-lg border',
				canManageGradebook && canManageEvaluation && 'sm:grid-cols-2'
			]}
		>
			{#if canManageGradebook}
				<div class={canManageEvaluation ? 'border-b sm:border-r sm:border-b-0' : ''}>
					<div class="flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5 text-sm font-medium">
						<ClipboardCheck class="size-4" /> คะแนนรายวิชา
					</div>
					<div class="divide-y">
						{#each gradebookControls as control (control.id)}
							<div class="flex items-center justify-between gap-3 px-4 py-3">
								<Label for={`gradebook-control-${control.id}`}
									>{phaseLabels[control.phaseCode as GradebookPhaseCode] ??
										control.phaseCode}</Label
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
								<Label for={`evaluation-control-${control.id}`}
									>{domainLabels[control.domain]}</Label
								>
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
		</div>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (open = false)}>ปิด</Button></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
