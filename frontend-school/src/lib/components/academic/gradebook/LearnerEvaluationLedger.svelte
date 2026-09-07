<script lang="ts">
	import type {
		LearnerEvaluationCriterion,
		LearnerEvaluationWorkspace
	} from '$lib/api/academicLearnerEvaluations';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Select from '$lib/components/ui/select';
	import { CheckCheck, ListChecks, Smartphone } from 'lucide-svelte';

	let {
		workspace,
		values,
		selectedCriterionIds,
		disabled = false,
		onselectionchange,
		onchange,
		onopencriteria,
		onopenmobile,
		onconfirm
	}: {
		workspace: LearnerEvaluationWorkspace;
		values: Record<string, number | null>;
		selectedCriterionIds: string[];
		disabled?: boolean;
		onselectionchange: (ids: string[]) => void;
		onchange: (criterionId: string, studentId: string, value: number | null) => void;
		onopencriteria: () => void;
		onopenmobile: (criterionId: string, studentId: string) => void;
		onconfirm: () => void;
	} = $props();

	const qualityOptions = [
		{ value: '3', label: '3 · ดีเยี่ยม' },
		{ value: '2', label: '2 · ดี' },
		{ value: '1', label: '1 · ผ่าน' },
		{ value: '0', label: '0 · ไม่ผ่าน' },
		{ value: 'blank', label: 'ยังไม่ประเมิน' }
	];
	let activeCriteria = $derived(
		workspace.criteria
			.filter((criterion) => criterion.lifecycle === 'active')
			.toSorted((left, right) => left.displayOrder - right.displayOrder)
	);

	function key(studentId: string, criterionId: string): string {
		return `${studentId}:${criterionId}`;
	}

	function labelFor(value: number | null | undefined): string {
		return (
			qualityOptions.find((option) => option.value === String(value ?? 'blank'))?.label ??
			'ยังไม่ประเมิน'
		);
	}

	function toggleCriterion(criterionId: string, checked: boolean) {
		onselectionchange(
			checked
				? selectedCriterionIds.includes(criterionId)
					? selectedCriterionIds
					: [...selectedCriterionIds, criterionId]
				: selectedCriterionIds.filter((id) => id !== criterionId)
		);
	}

	function missingCount(): number {
		let count = 0;
		for (const student of workspace.students) {
			for (const criterion of activeCriteria) {
				if (values[key(student.studentAcademicYearId, criterion.id)] == null) count += 1;
			}
		}
		return count;
	}

	function changeValue(criterion: LearnerEvaluationCriterion, studentId: string, value: string) {
		onchange(criterion.id, studentId, value === 'blank' ? null : Number(value));
	}
</script>

<section class="overflow-hidden rounded-xl border bg-card" aria-label="ตารางประเมินผู้เรียน">
	<header
		class="flex flex-col gap-3 border-b px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
	>
		<div>
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="font-semibold">ประเมินทุกหัวข้อของรายวิชา</h2>
				<Badge variant={missingCount() === 0 ? 'secondary' : 'outline'}>
					{missingCount() === 0 ? 'กรอกครบแล้ว' : `ยังว่าง ${missingCount()} ช่อง`}
				</Badge>
			</div>
			<p class="mt-0.5 text-sm text-muted-foreground">ผล 0–3 แยกตามรหัสวิชาและกลุ่มเรียน</p>
		</div>
		<div class="flex flex-wrap gap-2">
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || !workspace.canManage || activeCriteria.length === 0}
				onclick={() => onselectionchange(activeCriteria.map((criterion) => criterion.id))}
				>เลือกทุกช่อง</Button
			>
			<Button
				variant="outline"
				size="sm"
				disabled={disabled || !workspace.canManage || selectedCriterionIds.length === 0}
				onclick={() => onselectionchange([])}>ล้างการเลือก</Button
			>
			{#if workspace.canManage}
				<Button
					variant="outline"
					size="sm"
					disabled={disabled || workspace.locked}
					onclick={onopencriteria}
				>
					<ListChecks class="size-4" /> จัดหัวข้อประเมิน
				</Button>
			{/if}
		</div>
	</header>

	{#if activeCriteria.length === 0}
		<div class="flex min-h-44 flex-col items-center justify-center px-6 text-center">
			<p class="font-medium">ยังไม่มีหัวข้อประเมินสำหรับรายวิชานี้</p>
			<p class="mt-1 text-sm text-muted-foreground">
				ผู้รับผิดชอบรายวิชาสามารถนำหัวข้อของโรงเรียนมาใช้หรือเพิ่มหัวข้อได้
			</p>
		</div>
	{:else}
		<div class="hidden overflow-x-auto md:block">
			<table class="min-w-max border-collapse text-sm">
				<thead>
					<tr class="border-b bg-muted/30">
						<th class="sticky left-0 z-20 w-12 min-w-12 bg-muted px-3 py-3 text-center font-medium"
							>ที่</th
						>
						<th
							class="sticky left-12 z-20 w-56 min-w-56 border-r bg-muted px-3 py-3 text-left font-medium"
							>นักเรียน</th
						>
						{#each activeCriteria as criterion (criterion.id)}
							<th
								class={[
									'w-44 min-w-44 border-r px-3 py-2 align-top',
									selectedCriterionIds.includes(criterion.id) &&
										'bg-primary/10 shadow-[inset_0_3px_0_hsl(var(--primary))]'
								]}
							>
								<label class="flex cursor-pointer items-start gap-2 text-left">
									<Checkbox
										checked={selectedCriterionIds.includes(criterion.id)}
										disabled={disabled || !workspace.canManage}
										onCheckedChange={(checked) => toggleCriterion(criterion.id, checked === true)}
									/>
									<span class="line-clamp-2 font-medium" title={criterion.name}
										>{criterion.name}</span
									>
								</label>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each workspace.students as student, index (student.studentAcademicYearId)}
						<tr class="border-b last:border-b-0 hover:bg-muted/20">
							<td class="sticky left-0 z-10 bg-card px-3 py-2 text-center text-muted-foreground"
								>{index + 1}</td
							>
							<td class="sticky left-12 z-10 border-r bg-card px-3 py-2 font-medium"
								>{student.displayName}</td
							>
							{#each activeCriteria as criterion (criterion.id)}
								<td
									class={[
										'border-r p-1.5',
										selectedCriterionIds.includes(criterion.id) && 'bg-primary/5'
									]}
								>
									<Select.Root
										type="single"
										value={String(
											values[key(student.studentAcademicYearId, criterion.id)] ?? 'blank'
										)}
										disabled={disabled ||
											!workspace.canManage ||
											!selectedCriterionIds.includes(criterion.id)}
										onValueChange={(value) =>
											changeValue(criterion, student.studentAcademicYearId, value)}
									>
										<Select.Trigger class="h-8 w-40 text-xs"
											>{labelFor(
												values[key(student.studentAcademicYearId, criterion.id)]
											)}</Select.Trigger
										>
										<Select.Content>
											{#each qualityOptions as option (option.value)}
												<Select.Item value={option.value}>{option.label}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="divide-y md:hidden">
			{#each workspace.students as student, index (student.studentAcademicYearId)}
				<div class="flex items-center gap-3 px-4 py-3">
					<span
						class="flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium"
						>{index + 1}</span
					>
					<p class="min-w-0 flex-1 truncate font-medium">{student.displayName}</p>
					<Button
						variant="outline"
						size="sm"
						disabled={disabled || !workspace.canManage || selectedCriterionIds.length === 0}
						onclick={() => onopenmobile(selectedCriterionIds[0]!, student.studentAcademicYearId)}
					>
						<Smartphone class="size-4" /> ประเมิน
					</Button>
				</div>
			{/each}
		</div>
	{/if}

	<footer
		class="flex flex-col gap-2 border-t bg-muted/20 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
	>
		<p class="text-sm text-muted-foreground">
			{workspace.confirmationIsCurrent
				? 'ยืนยันผลประเมินกลุ่มนี้แล้ว'
				: 'ต้องกรอกครบทุกหัวข้อก่อนยืนยัน'}
		</p>
		<Button disabled={disabled || !workspace.canConfirm || missingCount() > 0} onclick={onconfirm}>
			<CheckCheck class="size-4" /> บันทึกผลประเมินและยืนยัน
		</Button>
	</footer>
</section>
