<script lang="ts">
	import type {
		AcademicTerm,
		AcademicTermType,
		AcademicYear,
		BellSchedule,
		BellSchedulePeriod,
		ReplaceBellSchedulePeriodsRequest,
		UpdateAcademicTermRequest,
		UpdateAcademicYearRequest
	} from '$lib/api/academic-core';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { CalendarPlus, ChevronRight, Pencil, Plus, Trash2, X } from 'lucide-svelte';

	type YearDraft = {
		year: number;
		name: string;
		startDate: string;
		endDate: string;
	};

	type TermDraft = {
		academicYearId: string;
		sequence: number;
		code: string;
		name: string;
		termType: AcademicTermType;
		startDate: string;
		endDate: string;
		includedInYearResult: boolean;
		blocksYearClosure: boolean;
		bellScheduleId: string;
	};

	type BellScheduleDraft = {
		academicYearId: string;
		code: string;
		name: string;
		isDefault: boolean;
	};

	type PeriodDraft = ReplaceBellSchedulePeriodsRequest['periods'][number] & {
		applicableDaysText: string;
	};

	const TERM_TYPE_OPTIONS: Array<{ value: AcademicTermType; label: string }> = [
		{ value: 'regular', label: 'ปกติ' },
		{ value: 'summer', label: 'ฤดูร้อน' },
		{ value: 'remedial', label: 'ซ่อมเสริม' },
		{ value: 'custom', label: 'กำหนดเอง' }
	];

	let {
		years,
		termsByYear,
		bellSchedules = [],
		canManage = false,
		busy = false,
		onCreateYear,
		onUpdateYear,
		onCreateBellSchedule,
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
		onCreateYear: (draft: YearDraft) => Promise<void>;
		onUpdateYear: (id: string, draft: UpdateAcademicYearRequest) => Promise<void>;
		onCreateBellSchedule: (draft: BellScheduleDraft) => Promise<void>;
		onLoadBellSchedulePeriods: (id: string) => Promise<BellSchedulePeriod[]>;
		onReplaceBellSchedulePeriods: (
			id: string,
			draft: ReplaceBellSchedulePeriodsRequest
		) => Promise<BellSchedulePeriod[]>;
		onCreateTerm: (draft: TermDraft) => Promise<void>;
		onUpdateTerm: (id: string, draft: UpdateAcademicTermRequest) => Promise<void>;
	} = $props();

	let yearDraft = $state<YearDraft>({
		year: new Date().getFullYear() + 544,
		name: '',
		startDate: '',
		endDate: ''
	});
	let termDraft = $state<TermDraft>({
		academicYearId: '',
		sequence: 1,
		code: '1',
		name: 'ภาคเรียนที่ 1',
		termType: 'regular',
		startDate: '',
		endDate: '',
		includedInYearResult: true,
		blocksYearClosure: true,
		bellScheduleId: ''
	});
	let bellScheduleDraft = $state<BellScheduleDraft>({
		academicYearId: '',
		code: 'DEFAULT',
		name: 'ตารางเวลาปกติ',
		isDefault: true
	});
	let yearEdit = $state<(UpdateAcademicYearRequest & { id: string }) | null>(null);
	let termEdit = $state<
		(UpdateAcademicTermRequest & { id: string; academicYearId: string }) | null
	>(null);
	let selectedBellScheduleId = $state('');
	let periodDrafts = $state<PeriodDraft[]>([]);
	let loadingPeriods = $state(false);
	let formError = $state('');

	const planningBellSchedules = $derived(
		bellSchedules.filter((schedule) =>
			years.some((year) => year.id === schedule.academicYearId && year.status === 'planning')
		)
	);

	function statusLabel(status: AcademicYear['status'] | AcademicTerm['status']) {
		return {
			planning: 'กำลังวางแผน',
			ready: 'พร้อมใช้งาน',
			active: 'กำลังใช้งาน',
			closing: 'กำลังปิดรอบ',
			closed: 'ปิดแล้ว',
			archived: 'เก็บถาวร',
			cancelled: 'ยกเลิก'
		}[status];
	}

	async function submitYear(event: SubmitEvent) {
		event.preventDefault();
		formError = '';
		if (!yearDraft.startDate || !yearDraft.endDate) {
			formError = 'กรุณาเลือกวันเริ่มและวันสิ้นสุดปีการศึกษา';
			return;
		}
		try {
			await onCreateYear(yearDraft);
			yearDraft = { ...yearDraft, name: '', startDate: '', endDate: '' };
		} catch (error) {
			formError = error instanceof Error ? error.message : 'สร้างปีการศึกษาไม่สำเร็จ';
		}
	}

	async function submitTerm(event: SubmitEvent) {
		event.preventDefault();
		formError = '';
		if (
			!termDraft.academicYearId ||
			!termDraft.startDate ||
			!termDraft.endDate ||
			!termDraft.bellScheduleId
		) {
			formError = 'กรุณาเลือกปี วันที่ และตารางเวลาให้ครบ';
			return;
		}
		try {
			await onCreateTerm(termDraft);
			termDraft = {
				...termDraft,
				sequence: termDraft.sequence + 1,
				code: '',
				name: '',
				startDate: '',
				endDate: ''
			};
		} catch (error) {
			formError = error instanceof Error ? error.message : 'สร้างภาคเรียนไม่สำเร็จ';
		}
	}

	async function submitBellSchedule(event: SubmitEvent) {
		event.preventDefault();
		formError = '';
		if (!bellScheduleDraft.academicYearId) {
			formError = 'กรุณาเลือกปีการศึกษา';
			return;
		}
		try {
			await onCreateBellSchedule(bellScheduleDraft);
			bellScheduleDraft = { ...bellScheduleDraft, code: '', name: '', isDefault: false };
		} catch (error) {
			formError = error instanceof Error ? error.message : 'สร้างตารางเวลาไม่สำเร็จ';
		}
	}

	function beginYearEdit(year: AcademicYear) {
		yearEdit = {
			id: year.id,
			year: year.year,
			name: year.name,
			startDate: year.startDate,
			endDate: year.endDate,
			schoolDays: [...year.schoolDays],
			rowVersion: year.rowVersion
		};
	}

	async function submitYearEdit(event: SubmitEvent) {
		event.preventDefault();
		if (!yearEdit) return;
		formError = '';
		if (!yearEdit.startDate || !yearEdit.endDate) {
			formError = 'กรุณาเลือกวันเริ่มและวันสิ้นสุดปีการศึกษา';
			return;
		}
		const { id, ...draft } = yearEdit;
		try {
			await onUpdateYear(id, draft);
			yearEdit = null;
		} catch (error) {
			formError = error instanceof Error ? error.message : 'แก้ไขปีการศึกษาไม่สำเร็จ';
		}
	}

	function beginTermEdit(term: AcademicTerm) {
		termEdit = {
			id: term.id,
			academicYearId: term.academicYearId,
			sequence: term.sequence,
			code: term.code,
			name: term.name,
			termType: term.termType,
			startDate: term.startDate,
			endDate: term.endDate,
			includedInYearResult: term.includedInYearResult,
			blocksYearClosure: term.blocksYearClosure,
			bellScheduleId: term.bellScheduleId,
			rowVersion: term.rowVersion
		};
	}

	async function submitTermEdit(event: SubmitEvent) {
		event.preventDefault();
		if (!termEdit) return;
		formError = '';
		if (!termEdit.startDate || !termEdit.endDate || !termEdit.bellScheduleId) {
			formError = 'กรุณาเลือกวันที่และตารางเวลาให้ครบ';
			return;
		}
		const { id, academicYearId: _academicYearId, ...draft } = termEdit;
		try {
			await onUpdateTerm(id, draft);
			termEdit = null;
		} catch (error) {
			formError = error instanceof Error ? error.message : 'แก้ไขภาคเรียนไม่สำเร็จ';
		}
	}

	function periodDraft(period?: BellSchedulePeriod): PeriodDraft {
		return {
			name: period?.name ?? '',
			startTime: period?.startTime.slice(0, 5) ?? '',
			endTime: period?.endTime.slice(0, 5) ?? '',
			orderIndex: period?.orderIndex ?? periodDrafts.length + 1,
			applicableDays: [],
			applicableDaysText: period?.applicableDays ?? 'MON,TUE,WED,THU,FRI',
			isActive: period?.isActive ?? true
		};
	}

	async function selectBellSchedule(id: string) {
		selectedBellScheduleId = id;
		periodDrafts = [];
		if (!id) return;
		loadingPeriods = true;
		formError = '';
		try {
			periodDrafts = (await onLoadBellSchedulePeriods(id)).map((period) => periodDraft(period));
		} catch (error) {
			formError = error instanceof Error ? error.message : 'โหลดคาบเรียนไม่สำเร็จ';
		} finally {
			loadingPeriods = false;
		}
	}

	function addPeriod() {
		periodDrafts = [...periodDrafts, periodDraft()];
	}

	function removePeriod(index: number) {
		periodDrafts = periodDrafts.filter((_, draftIndex) => draftIndex !== index);
	}

	async function submitPeriods(event: SubmitEvent) {
		event.preventDefault();
		const schedule = bellSchedules.find((item) => item.id === selectedBellScheduleId);
		if (!schedule) return;
		formError = '';
		try {
			const saved = await onReplaceBellSchedulePeriods(schedule.id, {
				rowVersion: schedule.rowVersion,
				periods: periodDrafts.map(({ applicableDaysText, ...period }) => ({
					...period,
					name: period.name?.trim() || null,
					applicableDays: applicableDaysText
						.split(',')
						.map((day) => day.trim().toUpperCase())
						.filter(Boolean)
				}))
			});
			periodDrafts = saved.map((period) => periodDraft(period));
		} catch (error) {
			formError = error instanceof Error ? error.message : 'บันทึกคาบเรียนไม่สำเร็จ';
		}
	}
</script>

<div class="grid gap-5 xl:grid-cols-[minmax(0,1.4fr)_minmax(320px,0.6fr)]">
	<section class="space-y-3" aria-label="รายการปีและภาคเรียน">
		{#each years as year (year.id)}
			{@const terms = termsByYear.get(year.id) ?? []}
			<article class="overflow-hidden rounded-xl border bg-card shadow-sm">
				<header
					class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/30 px-5 py-4"
				>
					<div>
						<div class="flex items-center gap-2">
							<h2 class="text-base font-semibold">{year.name}</h2>
							<Badge variant={year.status === 'active' ? 'default' : 'secondary'}
								>{statusLabel(year.status)}</Badge
							>
						</div>
						<p class="mt-1 text-sm text-muted-foreground">{year.startDate} – {year.endDate}</p>
					</div>
					<div class="flex items-center gap-2">
						{#if canManage && year.status === 'planning'}
							<Button size="sm" variant="outline" onclick={() => beginYearEdit(year)}>
								<Pencil class="size-3.5" /> แก้ไขปี
							</Button>
						{/if}
						<div class="rounded-lg border bg-background px-3 py-2 text-right">
							<p class="text-xl font-semibold tabular-nums">{terms.length}</p>
							<p class="text-[11px] text-muted-foreground">ภาคเรียนจากรายการจริง</p>
						</div>
					</div>
				</header>

				<div class="divide-y">
					{#each terms as term (term.id)}
						<div class="grid gap-3 px-5 py-3 sm:grid-cols-[48px_1fr_auto] sm:items-center">
							<div
								class="flex size-10 items-center justify-center rounded-full bg-primary/10 font-semibold text-primary"
							>
								{term.sequence}
							</div>
							<div>
								<p class="font-medium">
									{term.name} <span class="text-muted-foreground">· {term.code}</span>
								</p>
								<p class="text-xs text-muted-foreground">
									{term.startDate} – {term.endDate} · {term.termType}
								</p>
							</div>
							<div class="flex items-center gap-2">
								<Badge variant="outline">{statusLabel(term.status)}</Badge>
								{#if canManage && term.status === 'planning'}
									<Button
										size="icon-sm"
										variant="ghost"
										onclick={() => beginTermEdit(term)}
										aria-label={`แก้ไข ${term.name}`}
									>
										<Pencil class="size-3.5" />
									</Button>
								{:else}
									<ChevronRight class="size-4 text-muted-foreground" />
								{/if}
							</div>
						</div>
					{:else}
						<p class="px-5 py-6 text-sm text-muted-foreground">
							ยังไม่มีภาคเรียน เพิ่มภาคเรียนตามรอบที่โรงเรียนใช้จริงได้ทางแบบฟอร์มด้านข้าง
						</p>
					{/each}
				</div>
			</article>
		{:else}
			<div class="rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground">
				ยังไม่มีปีการศึกษา เริ่มจากสร้างปีสำหรับวางแผน
			</div>
		{/each}
	</section>

	<aside class="space-y-4">
		{#if canManage}
			{#if yearEdit}
				<form
					class="space-y-3 rounded-xl border border-primary/30 bg-card p-5 shadow-sm"
					onsubmit={submitYearEdit}
				>
					<div class="flex items-center justify-between gap-3">
						<h2 class="font-semibold">แก้ไขปีที่กำลังวางแผน</h2>
						<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							onclick={() => (yearEdit = null)}
							aria-label="ปิดแบบแก้ไขปี"><X class="size-4" /></Button
						>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="edit-year-value">ปี พ.ศ.</Label><Input
								id="edit-year-value"
								type="number"
								bind:value={yearEdit.year}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="edit-year-name">ชื่อปี</Label><Input
								id="edit-year-name"
								bind:value={yearEdit.name}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="edit-year-start">วันเริ่ม</Label><DatePicker
								id="edit-year-start"
								bind:value={yearEdit.startDate}
								ariaLabel="เลือกวันเริ่มปีการศึกษา"
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="edit-year-end">วันสิ้นสุด</Label><DatePicker
								id="edit-year-end"
								bind:value={yearEdit.endDate}
								ariaLabel="เลือกวันสิ้นสุดปีการศึกษา"
								required
							/>
						</div>
					</div>
					<Button type="submit" class="w-full" disabled={busy}>บันทึกปีการศึกษา</Button>
				</form>
			{/if}

			{#if termEdit}
				<form
					class="space-y-3 rounded-xl border border-primary/30 bg-card p-5 shadow-sm"
					onsubmit={submitTermEdit}
				>
					<div class="flex items-center justify-between gap-3">
						<h2 class="font-semibold">แก้ไขภาคเรียนที่กำลังวางแผน</h2>
						<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							onclick={() => (termEdit = null)}
							aria-label="ปิดแบบแก้ไขภาคเรียน"><X class="size-4" /></Button
						>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="edit-term-sequence">ลำดับ</Label><Input
								id="edit-term-sequence"
								type="number"
								min="1"
								bind:value={termEdit.sequence}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="edit-term-code">รหัส</Label><Input
								id="edit-term-code"
								bind:value={termEdit.code}
								required
							/>
						</div>
					</div>
					<div class="space-y-1.5">
						<Label for="edit-term-name">ชื่อภาคเรียน</Label><Input
							id="edit-term-name"
							bind:value={termEdit.name}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="edit-term-type">ประเภท</Label>
						<Select.Root type="single" bind:value={termEdit.termType}>
							<Select.Trigger id="edit-term-type" class="w-full">
								{TERM_TYPE_OPTIONS.find((option) => option.value === termEdit?.termType)?.label ??
									'เลือกประเภท'}
							</Select.Trigger>
							<Select.Content>
								{#each TERM_TYPE_OPTIONS as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="edit-term-start">วันเริ่ม</Label><DatePicker
								id="edit-term-start"
								bind:value={termEdit.startDate}
								ariaLabel="เลือกวันเริ่มภาคเรียน"
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="edit-term-end">วันสิ้นสุด</Label><DatePicker
								id="edit-term-end"
								bind:value={termEdit.endDate}
								ariaLabel="เลือกวันสิ้นสุดภาคเรียน"
								required
							/>
						</div>
					</div>
					<div class="space-y-1.5">
						<Label for="edit-term-bell">ตารางเวลา</Label>
						<Select.Root type="single" bind:value={termEdit.bellScheduleId}>
							<Select.Trigger id="edit-term-bell" class="w-full">
								{bellSchedules.find((schedule) => schedule.id === termEdit?.bellScheduleId)?.name ??
									'เลือกตารางเวลา'}
							</Select.Trigger>
							<Select.Content>
								{#each bellSchedules.filter((schedule) => schedule.academicYearId === termEdit?.academicYearId) as schedule (schedule.id)}
									<Select.Item value={schedule.id}>{schedule.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<label class="flex items-center gap-2 text-sm"
						><input type="checkbox" bind:checked={termEdit.includedInYearResult} /> รวมในผลลัพธ์ทั้งปี</label
					>
					<label class="flex items-center gap-2 text-sm"
						><input type="checkbox" bind:checked={termEdit.blocksYearClosure} /> ต้องปิดรอบนี้ก่อนปิดปี</label
					>
					<Button type="submit" class="w-full" disabled={busy}>บันทึกภาคเรียน</Button>
				</form>
			{/if}

			<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={submitYear}>
				<div class="flex items-center gap-2">
					<CalendarPlus class="size-5 text-primary" />
					<h2 class="font-semibold">เพิ่มปีสำหรับวางแผน</h2>
				</div>
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<Label for="academic-year-value">ปี พ.ศ.</Label><Input
							id="academic-year-value"
							type="number"
							bind:value={yearDraft.year}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="academic-year-name">ชื่อปี</Label><Input
							id="academic-year-name"
							bind:value={yearDraft.name}
							placeholder="ปีการศึกษา 2571"
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="academic-year-start">วันเริ่ม</Label><DatePicker
							id="academic-year-start"
							bind:value={yearDraft.startDate}
							ariaLabel="เลือกวันเริ่มปีการศึกษา"
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="academic-year-end">วันสิ้นสุด</Label><DatePicker
							id="academic-year-end"
							bind:value={yearDraft.endDate}
							ariaLabel="เลือกวันสิ้นสุดปีการศึกษา"
							required
						/>
					</div>
				</div>
				<Button type="submit" class="w-full" disabled={busy}
					><Plus class="size-4" /> เพิ่มปีการศึกษา</Button
				>
			</form>

			<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={submitBellSchedule}>
				<h2 class="font-semibold">เพิ่มตารางเวลาของปี</h2>
				<div class="space-y-1.5">
					<Label for="bell-year">ปีการศึกษา</Label>
					<Select.Root type="single" bind:value={bellScheduleDraft.academicYearId}>
						<Select.Trigger id="bell-year" class="w-full">
							{years.find((year) => year.id === bellScheduleDraft.academicYearId)?.name ??
								'เลือกปี'}
						</Select.Trigger>
						<Select.Content>
							{#each years.filter((year) => year.status === 'planning') as year (year.id)}
								<Select.Item value={year.id}>{year.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<Label for="bell-code">รหัส</Label><Input
							id="bell-code"
							bind:value={bellScheduleDraft.code}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="bell-name">ชื่อ</Label><Input
							id="bell-name"
							bind:value={bellScheduleDraft.name}
							required
						/>
					</div>
				</div>
				<label class="flex items-center gap-2 text-sm"
					><input type="checkbox" bind:checked={bellScheduleDraft.isDefault} /> ใช้เป็นตารางเริ่มต้นของปี</label
				>
				<Button type="submit" variant="outline" class="w-full" disabled={busy}
					><Plus class="size-4" /> เพิ่มตารางเวลา</Button
				>
			</form>

			<form
				id="bell-schedules"
				class="scroll-mt-24 space-y-3 rounded-xl border bg-card p-5 shadow-sm"
				onsubmit={submitPeriods}
			>
				<div>
					<h2 class="font-semibold">จัดคาบในตารางเวลา</h2>
					<p class="mt-1 text-xs text-muted-foreground">
						กำหนดลำดับ เวลา และวันที่ใช้จริง ก่อนนำไปผูกกับภาคเรียน
					</p>
				</div>
				<div class="space-y-1.5">
					<Label for="period-schedule">ตารางเวลาของปีที่กำลังวางแผน</Label>
					<Select.Root
						type="single"
						value={selectedBellScheduleId}
						onValueChange={(value) => void selectBellSchedule(value)}
					>
						<Select.Trigger id="period-schedule" class="w-full">
							{#if selectedBellScheduleId}
								{@const selectedSchedule = planningBellSchedules.find(
									(schedule) => schedule.id === selectedBellScheduleId
								)}
								{years.find((year) => year.id === selectedSchedule?.academicYearId)?.name} · {selectedSchedule?.name}
							{:else}
								เลือกตารางเวลา
							{/if}
						</Select.Trigger>
						<Select.Content>
							{#each planningBellSchedules as schedule (schedule.id)}
								<Select.Item value={schedule.id}>
									{years.find((year) => year.id === schedule.academicYearId)?.name} · {schedule.name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				{#if loadingPeriods}
					<p class="text-sm text-muted-foreground">กำลังโหลดคาบ…</p>
				{:else if selectedBellScheduleId}
					<div class="space-y-3">
						{#each periodDrafts as period, index (index)}
							<div class="space-y-2 rounded-lg border bg-muted/20 p-3">
								<div class="flex items-center justify-between gap-2">
									<p class="text-sm font-medium">คาบลำดับ {period.orderIndex}</p>
									<Button
										type="button"
										size="icon-sm"
										variant="ghost"
										onclick={() => removePeriod(index)}
										aria-label={`ลบคาบลำดับ ${period.orderIndex}`}><Trash2 class="size-4" /></Button
									>
								</div>
								<div class="grid grid-cols-[72px_1fr] gap-2">
									<Input
										type="number"
										min="1"
										aria-label="ลำดับคาบ"
										bind:value={period.orderIndex}
										required
									/>
									<Input
										aria-label="ชื่อคาบ"
										placeholder="คาบ 1 / พักกลางวัน"
										bind:value={period.name}
									/>
								</div>
								<div class="grid grid-cols-2 gap-2">
									<Input
										type="time"
										aria-label="เวลาเริ่มคาบ"
										bind:value={period.startTime}
										required
									/>
									<Input
										type="time"
										aria-label="เวลาสิ้นสุดคาบ"
										bind:value={period.endTime}
										required
									/>
								</div>
								<Input
									aria-label="วันที่ใช้คาบ"
									placeholder="MON,TUE,WED,THU,FRI"
									bind:value={period.applicableDaysText}
									required
								/>
								<label class="flex items-center gap-2 text-xs"
									><input type="checkbox" bind:checked={period.isActive} /> เปิดใช้คาบนี้</label
								>
							</div>
						{/each}
						<Button type="button" variant="outline" class="w-full" onclick={addPeriod}
							><Plus class="size-4" /> เพิ่มคาบ</Button
						>
						<Button type="submit" class="w-full" disabled={busy || periodDrafts.length === 0}
							>บันทึกคาบทั้งหมด</Button
						>
					</div>
				{/if}
			</form>

			<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={submitTerm}>
				<h2 class="font-semibold">เพิ่มภาคเรียน</h2>
				<div class="space-y-1.5">
					<Label for="term-year">ปีการศึกษา</Label>
					<Select.Root type="single" bind:value={termDraft.academicYearId}>
						<Select.Trigger id="term-year" class="w-full">
							{years.find((year) => year.id === termDraft.academicYearId)?.name ?? 'เลือกปี'}
						</Select.Trigger>
						<Select.Content>
							{#each years.filter((year) => year.status === 'planning') as year (year.id)}
								<Select.Item value={year.id}>{year.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<Label for="term-sequence">ลำดับ</Label><Input
							id="term-sequence"
							type="number"
							min="1"
							bind:value={termDraft.sequence}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="term-code">รหัส</Label><Input
							id="term-code"
							bind:value={termDraft.code}
							required
						/>
					</div>
				</div>
				<div class="space-y-1.5">
					<Label for="term-name">ชื่อภาคเรียน</Label><Input
						id="term-name"
						bind:value={termDraft.name}
						required
					/>
				</div>
				<div class="space-y-1.5">
					<Label for="term-type">ประเภท</Label>
					<Select.Root type="single" bind:value={termDraft.termType}>
						<Select.Trigger id="term-type" class="w-full">
							{TERM_TYPE_OPTIONS.find((option) => option.value === termDraft.termType)?.label ??
								'เลือกประเภท'}
						</Select.Trigger>
						<Select.Content>
							{#each TERM_TYPE_OPTIONS as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<Label for="term-start">วันเริ่ม</Label><DatePicker
							id="term-start"
							bind:value={termDraft.startDate}
							ariaLabel="เลือกวันเริ่มภาคเรียน"
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="term-end">วันสิ้นสุด</Label><DatePicker
							id="term-end"
							bind:value={termDraft.endDate}
							ariaLabel="เลือกวันสิ้นสุดภาคเรียน"
							required
						/>
					</div>
				</div>
				<div class="space-y-1.5">
					<Label for="term-bell">ตารางเวลา</Label>
					<Select.Root type="single" bind:value={termDraft.bellScheduleId}>
						<Select.Trigger id="term-bell" class="w-full">
							{bellSchedules.find((schedule) => schedule.id === termDraft.bellScheduleId)?.name ??
								'เลือกตารางเวลา'}
						</Select.Trigger>
						<Select.Content>
							{#each bellSchedules.filter((schedule) => schedule.academicYearId === termDraft.academicYearId) as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<label class="flex items-center gap-2 text-sm"
					><input type="checkbox" bind:checked={termDraft.includedInYearResult} /> รวมในผลลัพธ์ทั้งปี</label
				>
				<label class="flex items-center gap-2 text-sm"
					><input type="checkbox" bind:checked={termDraft.blocksYearClosure} /> ต้องปิดรอบนี้ก่อนปิดปี</label
				>
				<Button type="submit" variant="outline" class="w-full" disabled={busy}
					><Plus class="size-4" /> เพิ่มภาคเรียน</Button
				>
			</form>
		{/if}
		{#if formError}<p
				role="alert"
				class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
			>
				{formError}
			</p>{/if}
	</aside>
</div>
