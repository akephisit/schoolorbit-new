<script lang="ts">
	import { untrack } from 'svelte';
	import type {
		AcademicYear,
		AcademicTerm,
		BellSchedule,
		AcademicContextSettings,
		AcademicContextSettingsMutationResponse,
		UpdateAcademicContextSettingsRequest,
		CreateAcademicYearRequest,
		UpdateAcademicYearRequest,
		CreateAcademicTermRequest,
		CreateAcademicTermCopySchoolScheduleRequest,
		AcademicTermSchoolScheduleCopyResponse,
		BellSchedulePeriodsSnapshot,
		UpdateAcademicTermRequest
	} from '#lib/api/academic-core.js';
	import { academicStatusLabel } from '#lib/academic/lifecycle/year-management.js';
	import AcademicYearSetupStep from './setup/AcademicYearSetupStep.svelte';
	import AcademicTermSetupStep from './setup/AcademicTermSetupStep.svelte';
	import AcademicContextSettingsControls from './AcademicContextSettingsControls.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		years,
		termsByYear,
		bellSchedules,
		canReadTerms,
		canManageYears,
		canManageTerms,
		contextSettings,
		busy,
		onUpdateContextSettings,
		onReloadContextSettings,
		onCreateYear,
		onUpdateYear,
		onCreateTermCopy,
		onLoadBellSchedulePeriods,
		onCreateTerm,
		onUpdateTerm
	}: {
		years: AcademicYear[];
		termsByYear: Map<string, AcademicTerm[]>;
		bellSchedules: BellSchedule[];
		canReadTerms: boolean;
		canManageYears: boolean;
		canManageTerms: boolean;
		contextSettings: AcademicContextSettings | null;
		busy: { year: boolean; term: boolean };
		onUpdateContextSettings: (
			input: UpdateAcademicContextSettingsRequest
		) => Promise<AcademicContextSettingsMutationResponse>;
		onReloadContextSettings: () => Promise<void>;
		onCreateYear: (input: CreateAcademicYearRequest) => Promise<AcademicYear>;
		onUpdateYear: (id: string, input: UpdateAcademicYearRequest) => Promise<AcademicYear>;
		onCreateTermCopy: (
			input: CreateAcademicTermCopySchoolScheduleRequest
		) => Promise<AcademicTermSchoolScheduleCopyResponse>;
		onLoadBellSchedulePeriods: (
			id: string,
			options?: { signal?: AbortSignal }
		) => Promise<BellSchedulePeriodsSnapshot>;
		onCreateTerm: (input: CreateAcademicTermRequest) => Promise<AcademicTerm>;
		onUpdateTerm: (id: string, input: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
	} = $props();
	let selectedYearId = $state(untrack(() => years[0]?.id ?? ''));
	let creating = $state(false);
	let editing = $state(false);
	const selectedYear = $derived(
		years.find((year) => year.id === selectedYearId) ?? years[0] ?? null
	);
	const terms = $derived(selectedYear ? (termsByYear.get(selectedYear.id) ?? []) : []);
	const schedules = $derived(
		bellSchedules.filter((schedule) => schedule.academicYearId === selectedYear?.id)
	);
	const suggestedYear = $derived(
		years.length ? Math.max(...years.map((year) => year.year)) + 1 : new Date().getFullYear() + 543
	);
</script>

<section class="space-y-5 rounded-xl border bg-card p-4 sm:p-5" aria-label="ตั้งค่าปีและภาคเรียน">
	<div class="flex flex-wrap items-end justify-between gap-3">
		<div class="w-full max-w-sm space-y-2">
			<h2 class="text-lg font-semibold">ตั้งค่าปีและภาคเรียน</h2>
			<Label for="setup-year">ปีที่ต้องการตั้งค่า</Label><Select.Root
				type="single"
				value={selectedYear?.id ?? ''}
				onValueChange={(value) => {
					selectedYearId = value;
					creating = false;
					editing = false;
				}}
				><Select.Trigger id="setup-year" class="w-full"
					><span>{selectedYear?.name ?? 'ยังไม่มีปีการศึกษา'}</span></Select.Trigger
				><Select.Content
					>{#each years as year (year.id)}<Select.Item value={year.id}
							>{year.name} · {academicStatusLabel(year.status)}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		{#if canManageYears}<Button
				variant="outline"
				onclick={() => {
					creating = true;
					editing = false;
				}}>เพิ่มปีการศึกษา</Button
			>{/if}
	</div>
	{#if canManageYears && (creating || editing || !selectedYear)}<div
			class="space-y-3 rounded-lg border p-4"
		>
			{#key creating ? 'new' : selectedYear?.id}<AcademicYearSetupStep
					existing={creating ? null : selectedYear}
					{suggestedYear}
					busy={busy.year}
					onCreate={onCreateYear}
					onUpdate={onUpdateYear}
					onSaved={(year) => {
						selectedYearId = year.id;
						creating = false;
						editing = false;
					}}
				/>{/key}{#if selectedYear}<Button
					variant="outline"
					onclick={() => {
						creating = false;
						editing = false;
					}}>ยกเลิก</Button
				>{/if}
		</div>{/if}
	{#if selectedYear}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex items-center gap-2">
				<h3 class="font-medium">{selectedYear.name}</h3>
				{#if canReadTerms}<p class="text-sm text-muted-foreground">{terms.length} ภาคเรียน</p>{/if}
				<Badge variant="outline">{academicStatusLabel(selectedYear.status)}</Badge>
			</div>
			{#if canManageYears}<Button
					size="sm"
					variant="ghost"
					onclick={() => {
						editing = true;
						creating = false;
					}}>แก้ไขปีการศึกษา</Button
				>{/if}
		</div>
		{#if canReadTerms}<section class="space-y-4" aria-label="ภาคเรียน">
				<h3 class="font-semibold">ภาคเรียน</h3>
				{#if canManageTerms}{#key selectedYear.id}<AcademicTermSetupStep
							year={selectedYear}
							{years}
							allTerms={[...termsByYear.values()].flat()}
							allSchedules={bellSchedules}
							onCreateCopy={onCreateTermCopy}
							onLoadPeriods={onLoadBellSchedulePeriods}
							{schedules}
							{terms}
							busy={busy.term}
							onCreate={onCreateTerm}
							onUpdate={onUpdateTerm}
							onSaved={() => {}}
						/>{/key}{:else}<div class="space-y-2">
						{#each terms as term (term.id)}<div
								class="flex items-center justify-between rounded-lg border p-3"
							>
								<span>{term.name}</span><Badge variant="outline"
									>{academicStatusLabel(term.status)}</Badge
								>
							</div>{/each}
					</div>{/if}
			</section>{/if}
		{#if contextSettings}<AcademicContextSettingsControls
				year={selectedYear}
				{terms}
				settings={contextSettings}
				canManage={canManageYears && canManageTerms}
				onsave={onUpdateContextSettings}
				onreload={onReloadContextSettings}
			/>{/if}
	{/if}
</section>
