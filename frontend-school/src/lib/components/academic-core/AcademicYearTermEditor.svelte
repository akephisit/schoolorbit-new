<script lang="ts">
	import type {
		AcademicTerm,
		AcademicYear,
		BellSchedule,
		BellSchedulePeriod,
		CreateAcademicTermRequest,
		CreateAcademicYearRequest,
		CreateBellScheduleRequest,
		ReplaceBellSchedulePeriodsRequest,
		UpdateAcademicTermRequest,
		UpdateAcademicYearRequest,
		UpdateBellScheduleRequest
	} from '$lib/api/academic-core';
	import AcademicTermSetupStep from '$lib/components/academic-core/setup/AcademicTermSetupStep.svelte';
	import AcademicYearSetupStep from '$lib/components/academic-core/setup/AcademicYearSetupStep.svelte';
	import BellSchedulePeriodsStep from '$lib/components/academic-core/setup/BellSchedulePeriodsStep.svelte';
	import BellScheduleSetupStep from '$lib/components/academic-core/setup/BellScheduleSetupStep.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import {
		BookOpenCheck,
		CalendarDays,
		Check,
		ChevronRight,
		Plus,
		School,
		TimerReset
	} from 'lucide-svelte';

	type StepKey = 'year' | 'schedule' | 'periods' | 'term';

	let {
		years,
		termsByYear,
		bellSchedules = [],
		canManage = false,
		busy = false,
		onCreateYear,
		onUpdateYear,
		onCreateBellSchedule,
		onUpdateBellSchedule,
		onLoadBellSchedulePeriods,
		onReplaceBellSchedulePeriods,
		onCreateTerm,
		onUpdateTerm
	}: {
		years: AcademicYear[];
		termsByYear: Map<string, AcademicTerm[]>;
		bellSchedules?: BellSchedule[];
		canManage?: boolean;
		busy?: boolean;
		onCreateYear: (draft: CreateAcademicYearRequest) => Promise<AcademicYear>;
		onUpdateYear: (id: string, draft: UpdateAcademicYearRequest) => Promise<AcademicYear>;
		onCreateBellSchedule: (draft: CreateBellScheduleRequest) => Promise<BellSchedule>;
		onUpdateBellSchedule: (id: string, draft: UpdateBellScheduleRequest) => Promise<BellSchedule>;
		onLoadBellSchedulePeriods: (id: string) => Promise<BellSchedulePeriod[]>;
		onReplaceBellSchedulePeriods: (
			id: string,
			draft: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriod[]>;
		onCreateTerm: (draft: CreateAcademicTermRequest) => Promise<AcademicTerm>;
		onUpdateTerm: (id: string, draft: UpdateAcademicTermRequest) => Promise<AcademicTerm>;
	} = $props();

	let selectedYearId = $derived(
		years.find((year) => year.status === 'planning')?.id ?? years[0]?.id ?? ''
	);
	let activeStep = $derived<StepKey>(selectedYearId ? firstIncompleteStep(selectedYearId) : 'year');
	let creatingYear = $derived(years.length === 0);
	let configuredScheduleIds = $state<string[]>([]);

	const selectedYear = $derived(years.find((year) => year.id === selectedYearId) ?? null);
	const selectedSchedules = $derived(
		bellSchedules.filter((schedule) => schedule.academicYearId === selectedYearId)
	);
	const selectedTerms = $derived(termsByYear.get(selectedYearId) ?? []);
	const suggestedYear = $derived(
		years.length === 0
			? new Date().getFullYear() + 543
			: Math.max(...years.map((year) => year.year)) + 1
	);

	function firstIncompleteStep(yearId: string): StepKey {
		const schedules = bellSchedules.filter((schedule) => schedule.academicYearId === yearId);
		if (schedules.length === 0) return 'schedule';
		if ((termsByYear.get(yearId) ?? []).length === 0) return 'periods';
		return 'term';
	}

	function selectYear(id: string) {
		selectedYearId = id;
		creatingYear = false;
		activeStep = firstIncompleteStep(id);
	}

	function startNewYear() {
		creatingYear = true;
		activeStep = 'year';
	}

	function editSelectedYear() {
		creatingYear = false;
		activeStep = 'year';
	}

	function handleYearSaved(year: AcademicYear) {
		selectedYearId = year.id;
		creatingYear = false;
		activeStep = 'schedule';
	}

	function handleScheduleSaved(_schedule: BellSchedule) {
		activeStep = 'periods';
	}

	function handlePeriodsSaved(scheduleId: string, _periods: BellSchedulePeriod[]) {
		if (!configuredScheduleIds.includes(scheduleId)) {
			configuredScheduleIds = [...configuredScheduleIds, scheduleId];
		}
		activeStep = 'term';
	}

	function handleTermSaved(_term: AcademicTerm) {
		activeStep = 'term';
	}

	function statusLabel(status: AcademicYear['status']) {
		return {
			planning: 'ฉบับเตรียมการ',
			ready: 'พร้อมใช้งาน',
			active: 'กำลังใช้งาน',
			closing: 'กำลังปิดรอบ',
			closed: 'ปิดแล้ว',
			archived: 'เก็บถาวร'
		}[status];
	}
</script>

<div class="grid gap-5 xl:grid-cols-[280px_minmax(0,1fr)]">
	<aside class="space-y-3 xl:sticky xl:top-20 xl:self-start" aria-label="ปีการศึกษา">
		<div class="flex items-center justify-between gap-3">
			<div>
				<p class="text-sm font-semibold">ปีการศึกษา</p>
				<p class="text-xs text-muted-foreground">เลือกปีที่จะวางแผนหรือดูข้อมูลเดิม</p>
			</div>
			{#if canManage}
				<Button
					type="button"
					size="icon-sm"
					variant="outline"
					onclick={startNewYear}
					aria-label="เพิ่มปีการศึกษา"
				>
					<Plus class="size-4" />
				</Button>
			{/if}
		</div>
		<div class="space-y-2">
			{#each years as year (year.id)}
				<button
					type="button"
					class={[
						'w-full rounded-xl border p-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
						year.id === selectedYearId
							? 'border-primary bg-primary/[0.06]'
							: 'bg-card hover:bg-muted/40'
					]}
					onclick={() => selectYear(year.id)}
				>
					<span class="flex items-center justify-between gap-2">
						<strong class="truncate text-sm">{year.name}</strong>
						{#if year.id === selectedYearId}<Check class="size-4 shrink-0 text-primary" />{/if}
					</span>
					<span class="mt-1 block text-xs text-muted-foreground">
						{(termsByYear.get(year.id) ?? []).length} ภาคเรียน · {statusLabel(year.status)}
					</span>
				</button>
			{:else}
				<p class="rounded-xl border border-dashed p-4 text-sm text-muted-foreground">
					ยังไม่มีปีการศึกษา เริ่มจากขั้นที่ 1
				</p>
			{/each}
		</div>
	</aside>

	<section class="min-w-0 space-y-4" aria-label="ลำดับการตั้งค่างานวิชาการ">
		<div class="overflow-hidden rounded-2xl border bg-card">
			<div
				class="border-b bg-gradient-to-r from-primary/[0.09] via-primary/[0.03] to-transparent p-5 sm:p-6"
			>
				<div class="flex flex-wrap items-start justify-between gap-4">
					<div>
						<p class="text-xs font-semibold tracking-wide text-primary">เส้นทางตั้งค่างานวิชาการ</p>
						<h2 class="mt-1 text-xl font-semibold">
							{creatingYear
								? 'สร้างปีสำหรับวางแผน'
								: (selectedYear?.name ?? 'เริ่มตั้งค่าปีการศึกษา')}
						</h2>
						<p class="mt-2 max-w-2xl text-sm text-muted-foreground">
							ทำตามลำดับ 4 ขั้น ข้อมูลในขั้นก่อนหน้าจะถูกส่งต่อให้ขั้นถัดไปโดยอัตโนมัติ
						</p>
					</div>
					<Badge variant="secondary">ฉบับเตรียมการ</Badge>
				</div>
			</div>
			<div class="flex gap-3 border-b bg-muted/20 px-5 py-3 text-xs text-muted-foreground sm:px-6">
				<School class="mt-0.5 size-4 shrink-0 text-primary" aria-hidden="true" />
				<p>
					หน้านี้ใช้เตรียมโครงสร้างเท่านั้น การเปิดใช้ปี การปิดภาคเรียน
					และการเลื่อนชั้นเป็นขั้นตอนแยกเมื่อระบบผลการเรียนพร้อม
				</p>
			</div>
		</div>

		<article class="overflow-hidden rounded-2xl border bg-card">
			<header class="flex items-center gap-3 border-b p-4 sm:p-5">
				<div
					class="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary text-sm font-semibold text-primary-foreground"
				>
					1
				</div>
				<div class="min-w-0 flex-1">
					<h3 class="font-semibold">ปีการศึกษา</h3>
					<p class="text-xs text-muted-foreground">ปี พ.ศ. ช่วงวันที่ และวันเรียนปกติ</p>
				</div>
				{#if selectedYear && !creatingYear}
					<Badge variant="outline"><Check class="size-3" /> บันทึกแล้ว</Badge>
				{/if}
				{#if canManage && selectedYear && activeStep !== 'year'}
					<Button type="button" size="sm" variant="ghost" onclick={editSelectedYear}>แก้ไข</Button>
				{/if}
			</header>
			{#if canManage && activeStep === 'year'}
				<div class="p-4 sm:p-6">
					{#key creatingYear ? 'new-year' : (selectedYear?.id ?? 'year')}
						<AcademicYearSetupStep
							existing={creatingYear ? null : selectedYear}
							{suggestedYear}
							{busy}
							onCreate={onCreateYear}
							onUpdate={onUpdateYear}
							onSaved={handleYearSaved}
						/>
					{/key}
				</div>
			{:else if selectedYear}
				<div class="grid gap-3 p-4 text-sm sm:grid-cols-3 sm:p-5">
					<div>
						<span class="text-muted-foreground">ชื่อ</span><strong class="block"
							>{selectedYear.name}</strong
						>
					</div>
					<div>
						<span class="text-muted-foreground">ช่วงวันที่</span><strong class="block"
							>{selectedYear.startDate} – {selectedYear.endDate}</strong
						>
					</div>
					<div>
						<span class="text-muted-foreground">วันเรียน</span><strong class="block"
							>{selectedYear.schoolDays.length} วันต่อสัปดาห์</strong
						>
					</div>
				</div>
			{/if}
		</article>

		<article id="bell-schedules" class="scroll-mt-20 overflow-hidden rounded-2xl border bg-card">
			<header class="flex items-center gap-3 border-b p-4 sm:p-5">
				<div
					class="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-sm font-semibold text-primary"
				>
					2
				</div>
				<div class="min-w-0 flex-1">
					<h3 class="font-semibold">ตารางเวลา</h3>
					<p class="text-xs text-muted-foreground">ตั้งชื่อชุดเวลาและเลือกตารางหลักของปี</p>
				</div>
				{#if selectedSchedules.length > 0}
					<Badge variant="outline">{selectedSchedules.length} ตาราง</Badge>
				{/if}
				{#if canManage && selectedYear && activeStep !== 'schedule'}
					<Button type="button" size="sm" variant="ghost" onclick={() => (activeStep = 'schedule')}
						>จัดการ</Button
					>
				{/if}
			</header>
			{#if !selectedYear}
				<p class="p-5 text-sm text-muted-foreground">บันทึกปีการศึกษาก่อนจึงจะเพิ่มตารางเวลาได้</p>
			{:else if canManage && activeStep === 'schedule'}
				<div class="p-4 sm:p-6">
					<BellScheduleSetupStep
						year={selectedYear}
						schedules={selectedSchedules}
						{busy}
						onCreate={onCreateBellSchedule}
						onUpdate={onUpdateBellSchedule}
						onSaved={handleScheduleSaved}
					/>
				</div>
			{:else if selectedSchedules.length > 0}
				<div class="flex flex-wrap gap-2 p-4 sm:p-5">
					{#each selectedSchedules as schedule (schedule.id)}
						<Badge variant={schedule.isDefault ? 'default' : 'secondary'}>
							{schedule.name}{schedule.isDefault ? ' · ตารางหลัก' : ''}
						</Badge>
					{/each}
				</div>
			{/if}
		</article>

		<article class="overflow-hidden rounded-2xl border bg-card">
			<header class="flex items-center gap-3 border-b p-4 sm:p-5">
				<div
					class="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-sm font-semibold text-primary"
				>
					3
				</div>
				<div class="min-w-0 flex-1">
					<h3 class="font-semibold">คาบเรียน</h3>
					<p class="text-xs text-muted-foreground">กำหนดเวลาและวันที่ใช้ของแต่ละคาบ</p>
				</div>
				{#if configuredScheduleIds.length > 0}
					<Badge variant="outline"><Check class="size-3" /> บันทึกแล้ว</Badge>
				{/if}
				{#if canManage && selectedSchedules.length > 0 && activeStep !== 'periods'}
					<Button type="button" size="sm" variant="ghost" onclick={() => (activeStep = 'periods')}
						>จัดการ</Button
					>
				{/if}
			</header>
			{#if selectedSchedules.length === 0}
				<p class="p-5 text-sm text-muted-foreground">เพิ่มตารางเวลาก่อนจึงจะจัดคาบเรียนได้</p>
			{:else if selectedYear && canManage && activeStep === 'periods'}
				<div class="p-4 sm:p-6">
					<BellSchedulePeriodsStep
						year={selectedYear}
						schedules={selectedSchedules}
						{busy}
						onLoad={onLoadBellSchedulePeriods}
						onReplace={onReplaceBellSchedulePeriods}
						onSaved={handlePeriodsSaved}
					/>
				</div>
			{:else}
				<p class="p-5 text-sm text-muted-foreground">
					เลือก “จัดการ” เพื่อตรวจหรือแก้คาบของแต่ละตารางเวลา
				</p>
			{/if}
		</article>

		<article class="overflow-hidden rounded-2xl border bg-card">
			<header class="flex items-center gap-3 border-b p-4 sm:p-5">
				<div
					class="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-sm font-semibold text-primary"
				>
					4
				</div>
				<div class="min-w-0 flex-1">
					<h3 class="font-semibold">ภาคเรียน</h3>
					<p class="text-xs text-muted-foreground">ผูกช่วงเรียนเข้ากับตารางเวลาที่เตรียมไว้</p>
				</div>
				{#if selectedTerms.length > 0}
					<Badge variant="outline">{selectedTerms.length} ภาคเรียน</Badge>
				{/if}
				{#if canManage && selectedSchedules.length > 0 && activeStep !== 'term'}
					<Button type="button" size="sm" variant="ghost" onclick={() => (activeStep = 'term')}
						>จัดการ</Button
					>
				{/if}
			</header>
			{#if selectedSchedules.length === 0}
				<p class="p-5 text-sm text-muted-foreground">เตรียมตารางเวลาและคาบเรียนก่อนเพิ่มภาคเรียน</p>
			{:else if selectedYear && canManage && activeStep === 'term'}
				<div class="p-4 sm:p-6">
					<AcademicTermSetupStep
						year={selectedYear}
						schedules={selectedSchedules}
						terms={selectedTerms}
						{busy}
						onCreate={onCreateTerm}
						onUpdate={onUpdateTerm}
						onSaved={handleTermSaved}
					/>
				</div>
			{:else if selectedTerms.length > 0}
				<div class="divide-y">
					{#each selectedTerms as term (term.id)}
						<div class="flex items-center justify-between gap-3 p-4 text-sm sm:px-5">
							<div>
								<strong>{term.name}</strong>
								<p class="text-xs text-muted-foreground">
									เริ่ม {term.startDate} · คาดว่าจะปิด {term.plannedEndDate ?? 'ยังไม่กำหนด'}
									{#if term.closedOn}
										· ปิดจริง {term.closedOn}{/if}
								</p>
							</div>
							<ChevronRight class="size-4 text-muted-foreground" />
						</div>
					{/each}
				</div>
			{/if}
		</article>

		<div
			class="grid gap-3 rounded-2xl border border-dashed bg-muted/10 p-4 text-sm sm:grid-cols-3 sm:p-5"
		>
			<div class="flex gap-3">
				<CalendarDays class="size-4 shrink-0 text-primary" />
				<p>
					<strong class="block">เปิดใช้งานภายหลัง</strong><span
						class="text-xs text-muted-foreground">เมื่อข้อมูลหลักสูตรและการจัดชั้นพร้อม</span
					>
				</p>
			</div>
			<div class="flex gap-3">
				<TimerReset class="size-4 shrink-0 text-primary" />
				<p>
					<strong class="block">ปิดภาคเรียนภายหลัง</strong><span
						class="text-xs text-muted-foreground">เมื่อคะแนนและผลการเรียนครบ</span
					>
				</p>
			</div>
			<div class="flex gap-3">
				<BookOpenCheck class="size-4 shrink-0 text-primary" />
				<p>
					<strong class="block">เลื่อนชั้นภายหลัง</strong><span
						class="text-xs text-muted-foreground">เป็น workflow แยก ไม่เกิดจากการสร้างปีใหม่</span
					>
				</p>
			</div>
		</div>
	</section>
</div>
