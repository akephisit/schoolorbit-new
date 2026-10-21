<script lang="ts">
	import type { CurriculumCreditCriterion, GradeLevelOption } from '#lib/api/academic-core.js';
	import {
		applyCreditCriteriaPreset,
		creditCriteriaPresets,
		sameCoveredGrades
	} from '#lib/academic-core/curriculum-credit-criteria.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		criteria,
		currentCriteria,
		grades,
		coveredGradeIds,
		saving,
		dirty,
		error,
		conflict,
		onchange,
		onsave,
		onreview,
		onreset
	}: {
		criteria: CurriculumCreditCriterion[];
		currentCriteria: readonly CurriculumCreditCriterion[];
		grades: readonly GradeLevelOption[];
		coveredGradeIds: readonly string[];
		saving: boolean;
		dirty: boolean;
		error: string;
		conflict: boolean;
		onchange: (criteria: CurriculumCreditCriterion[]) => void;
		onsave: () => void;
		onreview: () => void;
		onreset: () => void;
	} = $props();
	const scopes = [
		{ id: 'annual', label: 'รายปี' },
		{ id: 'level', label: 'ทั้งระดับ' }
	] as const;
	function update(scope: 'annual' | 'level', patch: Partial<CurriculumCreditCriterion>) {
		onchange(
			criteria.map((criterion) =>
				criterion.scope === scope ? { ...criterion, ...patch } : criterion
			)
		);
	}
</script>

<section class="space-y-3 rounded-xl border bg-card p-3" aria-label="เกณฑ์หน่วยกิตพื้นฐาน">
	<h3 class="font-semibold">เกณฑ์หน่วยกิตพื้นฐาน</h3>
	<p class="text-xs text-muted-foreground">
		ตรวจยอดที่จัดไว้ในโครงสร้างของแต่ละแผน ไม่ใช่ผลหน่วยกิตที่ผู้เรียนได้รับหรือการตัดสินจบการศึกษา
	</p>
	<p class="text-xs text-muted-foreground">
		ใช้กับ {coveredGradeIds
			.map((id) => grades.find((grade) => grade.id === id)?.short_name ?? 'ไม่พบชั้น')
			.join(' · ')} · ทุกภาคเรียน
	</p>
	{#each creditCriteriaPresets(grades, coveredGradeIds) as preset (preset.id)}
		<div class="space-y-1">
			<Button
				variant="outline"
				size="sm"
				class="h-auto whitespace-normal text-left"
				disabled={saving}
				onclick={() => onchange(applyCreditCriteriaPreset(criteria, preset.criterion))}
				>ใช้ตัวอย่าง {preset.label}</Button
			>
			<p class="text-xs text-muted-foreground">
				<a
					href="https://oho.ipst.ac.th/obec-cs-dt/"
					target="_blank"
					rel="noreferrer"
					class="underline underline-offset-2"
					>ตัวอย่างตามหลักสูตรแกนกลาง 2551 / คำสั่ง สพฐ. 922/2561</a
				>
				· ปรับแก้ได้ เปลี่ยนเฉพาะเกณฑ์{preset.criterion.scope === 'annual' ? 'รายปี' : 'ทั้งระดับ'} และยังไม่บันทึก
			</p>
		</div>
	{/each}
	{#if !criteria.length}<p class="text-sm text-muted-foreground">ยังไม่ได้กำหนดเกณฑ์</p>{/if}
	{#each scopes as scope (scope.id)}
		{@const criterion = criteria.find((item) => item.scope === scope.id)}
		{#if criterion}
			<div class="space-y-2 rounded-lg border p-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<span class="font-medium">พื้นฐาน{scope.label}</span><Button
						variant="ghost"
						size="sm"
						disabled={saving}
						onclick={() => onchange(criteria.filter((item) => item.scope !== scope.id))}
						>ลบเกณฑ์{scope.label}</Button
					>
				</div>
				<div class="flex items-center gap-2">
					<Checkbox
						id={`credit-enabled-${scope.id}`}
						checked={criterion.enabled}
						disabled={saving}
						onCheckedChange={(enabled) => update(scope.id, { enabled: enabled === true })}
					/><Label for={`credit-enabled-${scope.id}`}>เปิดตรวจเกณฑ์{scope.label}</Label>
				</div>
				<div class="grid gap-3 sm:grid-cols-2">
					<div class="space-y-1">
						<Label for={`credit-comparison-${scope.id}`}>การเปรียบเทียบ{scope.label}</Label
						><Select.Root
							type="single"
							value={criterion.comparison}
							onValueChange={(value) => {
								if (value === 'equal' || value === 'minimum')
									update(scope.id, { comparison: value });
							}}
							><Select.Trigger id={`credit-comparison-${scope.id}`} disabled={saving}
								>{criterion.comparison === 'equal' ? 'เท่ากับ' : 'อย่างน้อย'}</Select.Trigger
							><Select.Content
								><Select.Item value="equal">เท่ากับ</Select.Item><Select.Item value="minimum"
									>อย่างน้อย</Select.Item
								></Select.Content
							></Select.Root
						>
					</div>
					<div class="space-y-1">
						<Label for={`credit-target-${scope.id}`}>หน่วยกิตพื้นฐาน{scope.label}</Label><Input
							id={`credit-target-${scope.id}`}
							inputmode="decimal"
							value={criterion.targetCredits}
							disabled={saving}
							oninput={(event) => update(scope.id, { targetCredits: event.currentTarget.value })}
						/>
					</div>
				</div>
				{#if !sameCoveredGrades(criterion.gradeLevelIds, coveredGradeIds)}<p
						class="text-sm text-warning"
					>
						ชั้นที่ครอบคลุมเปลี่ยนไป ตรวจสอบเกณฑ์แล้วบันทึกเพื่อใช้กับชั้นที่แสดงข้างต้น
					</p>{/if}
			</div>
		{:else}<Button
				variant="outline"
				size="sm"
				disabled={saving}
				onclick={() =>
					onchange([
						...criteria,
						{
							scope: scope.id,
							comparison: 'equal',
							targetCredits: '0.00',
							enabled: true,
							gradeLevelIds: [...coveredGradeIds]
						}
					])}>เพิ่มเกณฑ์{scope.label}</Button
			>{/if}
	{/each}
	<p class="text-xs text-muted-foreground" data-testid="current-credit-criteria">
		เกณฑ์ปัจจุบัน: {currentCriteria.length
			? currentCriteria
					.map(
						(criterion) =>
							`${criterion.scope === 'annual' ? 'รายปี' : 'ทั้งระดับ'} ${criterion.comparison === 'equal' ? 'เท่ากับ' : 'อย่างน้อย'} ${criterion.targetCredits} หน่วยกิต (${criterion.enabled ? 'เปิด' : 'ปิด'})`
					)
					.join(' · ')
			: 'ยังไม่ได้กำหนด'}
	</p>
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	{#if conflict}<p class="text-sm text-muted-foreground">
			เกณฑ์เปลี่ยนไป ร่างเกณฑ์และร่างส่วนอื่นยังอยู่ โหลดเกณฑ์ล่าสุดแล้วตรวจสอบก่อนบันทึกอีกครั้ง
		</p>
		<LoadingButton variant="outline" loading={saving} onclick={onreview}
			>โหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ</LoadingButton
		>{/if}
	<div class="flex flex-wrap justify-end gap-2">
		<Button variant="outline" disabled={saving} onclick={onreset}>ใช้เกณฑ์ปัจจุบัน</Button
		><LoadingButton loading={saving} disabled={!dirty || conflict} onclick={onsave}
			>บันทึกเกณฑ์หน่วยกิต</LoadingButton
		>
	</div>
</section>
