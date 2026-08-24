<script lang="ts">
	import { onMount } from 'svelte';
	import type { Cell, Row, Workbook, Worksheet } from 'exceljs';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		listAcademicYears,
		listBellSchedulePeriods,
		listBellSchedules,
		listHomerooms,
		type BellSchedule,
		type BellSchedulePeriod,
		type Homeroom
	} from '$lib/api/academic-core';
	import {
		listLearningGroups,
		listLearningOfferings,
		type LearningGroup,
		type LearningOffering
	} from '$lib/api/learning-delivery';
	import { lookupRooms, type RoomLookupItem } from '$lib/api/lookup';
	import {
		createTimetableEntry,
		deleteTimetableEntry,
		listTimetableEntries,
		updateTimetableEntry,
		type TimetableEntry
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '$lib/stores/timetable-socket';
	import {
		buildTeacherLoadExportRows,
		calculateTeacherLoadColumnWidths,
		TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS,
		TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS,
		type TeacherLoadExportRows
	} from '$lib/utils/timetable-teacher-load-export';
	import {
		CalendarClock,
		FileSpreadsheet,
		Loader2,
		Plus,
		RotateCcw,
		Save,
		Trash2
	} from 'lucide-svelte';

	type ViewKind = 'learning_group' | 'homeroom';
	type EntryType = 'COURSE' | 'ACTIVITY' | 'HOMEROOM' | 'ACADEMIC' | 'BREAK';

	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];
	const entryTypeOptions: Array<{ value: EntryType; label: string }> = [
		{ value: 'COURSE', label: 'รายวิชา' },
		{ value: 'ACTIVITY', label: 'กิจกรรม' },
		{ value: 'HOMEROOM', label: 'โฮมรูม' },
		{ value: 'ACADEMIC', label: 'กิจกรรมวิชาการ' },
		{ value: 'BREAK', label: 'พัก' }
	];

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let schedules = $state<BellSchedule[]>([]);
	let periods = $state<BellSchedulePeriod[]>([]);
	let offerings = $state<LearningOffering[]>([]);
	let groups = $state<LearningGroup[]>([]);
	let homerooms = $state<Homeroom[]>([]);
	let rooms = $state<RoomLookupItem[]>([]);
	let entries = $state<TimetableEntry[]>([]);
	let selectedScheduleId = $state('');
	let viewKind = $state<ViewKind>('learning_group');
	let selectedTargetId = $state('');
	let selectedEntryId = $state('');
	let formDay = $state('MON');
	let formPeriodId = $state('');
	let formRoomId = $state('');
	let formEntryType = $state<EntryType>('COURSE');
	let formTitle = $state('');
	let formNote = $state('');
	let loading = $state(false);
	let busy = $state(false);
	let isTeacherLoadExporting = $state(false);
	let dirty = $state(false);
	let errorMessage = $state('');
	let revision = 0;

	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_READ_ASSIGNED,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const selectedEntry = $derived(entries.find((entry) => entry.id === selectedEntryId) ?? null);
	const activeDays = $derived.by(() => {
		const selectedYear = $academicContext.options?.years.find((year) => year.id === academicYearId);
		void selectedYear;
		const configured = new Set(
			periods.flatMap((period) =>
				(period.applicableDays ?? '')
					.split(',')
					.map((day) => day.trim())
					.filter(Boolean)
			)
		);
		return configured.size > 0
			? dayOptions.filter((day) => configured.has(day.value))
			: dayOptions.slice(0, 5);
	});
	const visibleEntries = $derived(
		entries.filter((entry) => {
			if (entry.bellScheduleId !== selectedScheduleId) return false;
			return viewKind === 'learning_group'
				? entry.learningGroupId === selectedTargetId
				: entry.homeroomId === selectedTargetId;
		})
	);

	function offeringForGroup(group: LearningGroup): LearningOffering | undefined {
		return offerings.find((offering) => offering.id === group.learningOfferingId);
	}

	function groupLabel(group: LearningGroup): string {
		const offering = offeringForGroup(group);
		return `${offering?.codeSnapshot ?? ''} · ${group.code} ${group.name}`.trim();
	}

	function entryTitle(entry: TimetableEntry): string {
		return (
			entry.offeringCode ??
			entry.title ??
			entry.activityVersionDisplayLabel ??
			entry.subjectVersionDisplayLabel ??
			entry.entryType
		);
	}

	function entriesForCell(day: string, periodId: string): TimetableEntry[] {
		return visibleEntries.filter(
			(entry) => entry.dayOfWeek === day && entry.bellSchedulePeriodId === periodId
		);
	}

	async function loadPeriods(scheduleId: string): Promise<void> {
		periods = (await listBellSchedulePeriods(scheduleId)).sort(
			(a, b) => a.orderIndex - b.orderIndex
		);
		formPeriodId = periods[0]?.id ?? '';
	}

	async function loadWorkspace(termId: string, yearId: string): Promise<void> {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const years = await listAcademicYears();
			if (!years.some((year) => year.id === yearId)) throw new Error('ไม่พบปีการศึกษาที่เลือก');
			schedules = await listBellSchedules(yearId);
			const preferredSchedule = schedules.find((schedule) => schedule.isDefault) ?? schedules[0];
			selectedScheduleId = preferredSchedule?.id ?? '';
			periods = [];
			if (preferredSchedule) await loadPeriods(preferredSchedule.id);
			offerings = await listLearningOfferings(termId);
			const loadedGroups: LearningGroup[] = [];
			for (const offering of offerings) {
				const offeringGroups = await listLearningGroups(offering.id);
				loadedGroups.push(...offeringGroups);
			}
			groups = loadedGroups;
			homerooms = await listHomerooms(yearId);
			rooms = await lookupRooms({ activeOnly: true, limit: 500 });
			entries = await listTimetableEntries({ academicTermId: termId });
			if (current !== revision) return;
			viewKind = groups.length > 0 ? 'learning_group' : 'homeroom';
			selectedTargetId = groups[0]?.id ?? homerooms[0]?.id ?? '';
			resetForm();
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดพื้นที่จัดตารางสอนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function refreshEntries(): Promise<void> {
		if (!academicTermId) return;
		entries = await listTimetableEntries({ academicTermId });
	}

	async function changeSchedule(event: Event): Promise<void> {
		const nextId = (event.currentTarget as HTMLSelectElement).value;
		if (dirty) {
			(event.currentTarget as HTMLSelectElement).value = selectedScheduleId;
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนตารางเวลา');
			return;
		}
		selectedScheduleId = nextId;
		loading = true;
		try {
			await loadPeriods(nextId);
			resetForm();
		} finally {
			loading = false;
		}
	}

	function changeViewKind(nextKind: ViewKind): void {
		if (dirty) {
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนมุมมอง');
			return;
		}
		viewKind = nextKind;
		selectedTargetId =
			nextKind === 'learning_group' ? (groups[0]?.id ?? '') : (homerooms[0]?.id ?? '');
		resetForm();
	}

	function changeTarget(event: Event): void {
		if (dirty) {
			(event.currentTarget as HTMLSelectElement).value = selectedTargetId;
			toast.warning('กรุณาบันทึกหรือยกเลิกแบบร่างก่อนเปลี่ยนกลุ่ม');
			return;
		}
		selectedTargetId = (event.currentTarget as HTMLSelectElement).value;
		resetForm();
	}

	function resetForm(): void {
		selectedEntryId = '';
		formDay = activeDays[0]?.value ?? 'MON';
		formPeriodId = periods[0]?.id ?? '';
		formRoomId = '';
		formEntryType = viewKind === 'learning_group' ? 'COURSE' : 'HOMEROOM';
		formTitle = '';
		formNote = '';
		dirty = false;
	}

	function startAtCell(day: string, periodId: string): void {
		if (!canManage) return;
		resetForm();
		formDay = day;
		formPeriodId = periodId;
		dirty = true;
	}

	function editEntry(entry: TimetableEntry): void {
		selectedEntryId = entry.id;
		formDay = entry.dayOfWeek;
		formPeriodId = entry.bellSchedulePeriodId;
		formRoomId = entry.roomId ?? '';
		formEntryType = entry.entryType as EntryType;
		formTitle = entry.title ?? '';
		formNote = entry.note ?? '';
		dirty = false;
	}

	function markDirty(): void {
		dirty = true;
	}

	async function saveEntry(): Promise<void> {
		if (!academicTermId || !selectedTargetId || !formPeriodId) return;
		busy = true;
		errorMessage = '';
		try {
			if (selectedEntry) {
				await updateTimetableEntry(selectedEntry.id, {
					rowVersion: selectedEntry.rowVersion,
					dayOfWeek: formDay,
					bellSchedulePeriodId: formPeriodId,
					roomId: formRoomId || null,
					clearRoom: !formRoomId,
					note: formNote.trim() || null,
					clearNote: !formNote.trim(),
					title: formTitle.trim() || null
				});
			} else {
				const selectedGroup = groups.find((group) => group.id === selectedTargetId);
				await createTimetableEntry({
					academicTermId,
					learningGroupId: viewKind === 'learning_group' ? selectedTargetId : null,
					homeroomId: viewKind === 'homeroom' ? selectedTargetId : null,
					dayOfWeek: formDay,
					bellSchedulePeriodId: formPeriodId,
					roomId: formRoomId || null,
					note: formNote.trim() || null,
					entryType: formEntryType,
					title: formTitle.trim() || null,
					instructorIds: selectedGroup?.teacherAssignments.map((teacher) => teacher.teacherId) ?? []
				});
			}
			await refreshEntries();
			resetForm();
			toast.success('บันทึกคาบในตารางแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกคาบไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	async function removeEntry(): Promise<void> {
		if (!selectedEntry) return;
		busy = true;
		try {
			await deleteTimetableEntry(selectedEntry.id, selectedEntry.rowVersion);
			await refreshEntries();
			resetForm();
			toast.success('ลบคาบออกจากตารางแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ลบคาบไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	const teacherLoadFontName = 'TH Sarabun New';

	function styleTeacherLoadCell(cell: Cell, emphasized = false): void {
		cell.font = { name: teacherLoadFontName, size: 16, bold: emphasized };
		cell.alignment = { vertical: 'middle', wrapText: true };
		cell.border = {
			top: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			left: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			bottom: { style: 'thin', color: { argb: 'FFE2E8F0' } },
			right: { style: 'thin', color: { argb: 'FFE2E8F0' } }
		};
	}

	function styleTeacherLoadRow(row: Row, kind: 'header' | 'group' | 'detail'): void {
		row.height = kind === 'header' ? 28 : 24;
		row.eachCell({ includeEmpty: true }, (cell) => {
			styleTeacherLoadCell(cell, kind !== 'detail');
			if (kind !== 'detail') {
				cell.fill = {
					type: 'pattern',
					pattern: 'solid',
					fgColor: { argb: kind === 'header' ? 'FFE2E8F0' : 'FFF1F5F9' }
				};
			}
		});
	}

	function appendTeacherLoadSheet(
		workbook: Workbook,
		name: string,
		rows: Array<Array<string | number>>,
		widthOptions:
			| typeof TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
			| typeof TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
	): Worksheet {
		const worksheet = workbook.addWorksheet(name);
		worksheet.columns = calculateTeacherLoadColumnWidths(rows, widthOptions).map((width) => ({
			width
		}));
		worksheet.properties.defaultRowHeight = 24;

		for (const [index, values] of rows.entries()) {
			const row = worksheet.addRow(values);
			const firstCell = String(values[0] ?? '');
			styleTeacherLoadRow(
				row,
				index === 0 ? 'header' : firstCell.startsWith('กลุ่มสาระ:') ? 'group' : 'detail'
			);
		}

		worksheet.views = [{ state: 'frozen', ySplit: 1 }];
		worksheet.pageSetup = {
			orientation: 'landscape',
			fitToPage: true,
			fitToWidth: 1,
			fitToHeight: 0
		};
		return worksheet;
	}

	function appendTeacherLoadSummarySheet(
		workbook: Workbook,
		exportRows: TeacherLoadExportRows
	): void {
		appendTeacherLoadSheet(
			workbook,
			'สรุปต่อครู',
			exportRows.summarySheetRows,
			TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
		);
	}

	function appendTeacherLoadDetailSheet(
		workbook: Workbook,
		exportRows: TeacherLoadExportRows
	): void {
		appendTeacherLoadSheet(
			workbook,
			'รายละเอียด',
			exportRows.detailSheetRows,
			TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
		);
	}

	function safeFileName(value: string): string {
		return (
			value
				.replace(/[\\/:*?"<>|]/g, '-')
				.replace(/\s+/g, ' ')
				.trim() || 'สรุปคาบสอนครู'
		);
	}

	function saveTeacherLoadWorkbookBuffer(buffer: ArrayBuffer, fileName: string): void {
		const blob = new Blob([buffer], {
			type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
		});
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = fileName;
		document.body.appendChild(link);
		link.click();
		link.remove();
		URL.revokeObjectURL(url);
	}

	async function handleExportTeacherLoadXlsx(): Promise<void> {
		if (!academicTermId || isTeacherLoadExporting) return;
		const exportRows = buildTeacherLoadExportRows(entries);
		if (exportRows.summaryRows.length === 0) {
			toast.error('ไม่พบคาบสอนสำหรับภาคเรียนนี้');
			return;
		}

		isTeacherLoadExporting = true;
		try {
			const ExcelJSModule = await import('exceljs');
			const ExcelJS = ExcelJSModule.default;
			const workbook = new ExcelJS.Workbook();
			workbook.creator = 'SchoolOrbit';
			workbook.created = new Date();
			workbook.modified = new Date();
			appendTeacherLoadSummarySheet(workbook, exportRows);
			appendTeacherLoadDetailSheet(workbook, exportRows);

			const selectedTerm = $academicContext.options?.terms.find(
				(term) => term.id === academicTermId
			);
			const selectedYear = $academicContext.options?.years.find(
				(year) => year.id === academicYearId
			);
			const fileName = safeFileName(
				`สรุปคาบสอนครู-${selectedTerm?.name ?? 'ภาคเรียน'}-${selectedYear?.name ?? 'ปีการศึกษา'}`
			);
			const buffer = await workbook.xlsx.writeBuffer();
			saveTeacherLoadWorkbookBuffer(buffer, `${fileName}.xlsx`);
			toast.success(`ดาวน์โหลดสรุปคาบสอน ${exportRows.summaryRows.length} คนแล้ว`);
		} catch (error) {
			console.error('Failed to export teacher load workbook', error);
			toast.error('ส่งออกสรุปคาบสอนไม่สำเร็จ');
		} finally {
			isTeacherLoadExporting = false;
		}
	}

	onMount(() => {
		let selectedTermId = '';
		let selectedYearId = '';
		let currentUserId = '';
		let loadedContextKey = '';
		let connectedSocketKey = '';
		let observedRefresh: number | null = null;

		function synchronizeContext() {
			if (!selectedTermId || !selectedYearId) return;
			const contextKey = `${selectedYearId}:${selectedTermId}`;
			if (contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				void loadWorkspace(selectedTermId, selectedYearId);
			}
			if (!currentUserId) return;
			const socketKey = `${selectedTermId}:${currentUserId}`;
			if (socketKey !== connectedSocketKey) {
				connectedSocketKey = socketKey;
				connectTimetableSocket({ academicTermId: selectedTermId, currentUserId });
			}
		}

		const unregisterDirty = registerAcademicContextDirtySource('academic-timetable', () => dirty);
		const unsubscribeContext = academicContext.subscribe((state) => {
			selectedTermId = state.selected.academicTermId ?? '';
			selectedYearId = state.selected.academicYearId ?? '';
			synchronizeContext();
		});
		const unsubscribeAuth = authStore.subscribe((state) => {
			currentUserId = state.user?.id ?? '';
			synchronizeContext();
		});
		const unsubscribeRefresh = refreshTrigger.subscribe((value) => {
			if (observedRefresh === null) {
				observedRefresh = value;
				return;
			}
			if (value === observedRefresh) return;
			observedRefresh = value;
			if (!selectedTermId || !selectedYearId) return;
			if (dirty) {
				toast.warning(
					'มีการเปลี่ยนแปลงตารางจากผู้ใช้อื่น กรุณาบันทึกหรือยกเลิกแบบร่างก่อนโหลดใหม่'
				);
				return;
			}
			void loadWorkspace(selectedTermId, selectedYearId);
		});
		return () => {
			unsubscribeRefresh();
			unsubscribeAuth();
			unsubscribeContext();
			unregisterDirty();
			disconnectTimetableSocket();
		};
	});
</script>

<PageShell
	title="จัดตารางสอน"
	description="จัดคาบตามกลุ่มเรียนหรือห้องประจำชั้น โดยใช้ตารางเวลาและชุดการเรียนของภาคเรียนที่เลือก"
>
	{#snippet actions()}
		<Button
			variant="outline"
			disabled={isTeacherLoadExporting || loading || entries.length === 0 || !academicTermId}
			onclick={handleExportTeacherLoadXlsx}
		>
			{#if isTeacherLoadExporting}
				<Loader2 class="animate-spin" />
			{:else}
				<FileSpreadsheet />
			{/if}
			สรุปคาบ XLSX
		</Button>
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูตารางสอน"
			description="ต้องมีสิทธิ์อ่านชุดการเรียนที่เกี่ยวข้อง"
		/>
	{:else if !academicTermId || !academicYearId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && entries.length === 0}
		<PageState
			variant="error"
			title="โหลดตารางสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicTermId, academicYearId)}
		/>
	{:else if schedules.length === 0}
		<PageState
			title="ปีนี้ยังไม่มีตารางเวลา"
			description="สร้างตารางเวลาและคาบเรียนก่อนเริ่มจัดตารางสอน"
		/>
	{:else}
		<div class="space-y-5">
			<Card.Root class="gap-0 py-0">
				<Card.Content class="grid gap-4 pt-6 lg:grid-cols-[14rem_auto_minmax(16rem,1fr)]">
					<div class="space-y-2">
						<Label for="timetable-schedule">ตารางเวลา</Label>
						<select
							id="timetable-schedule"
							class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
							value={selectedScheduleId}
							onchange={changeSchedule}
						>
							{#each schedules as schedule (schedule.id)}
								<option value={schedule.id}>{schedule.code} · {schedule.name}</option>
							{/each}
						</select>
					</div>
					<div class="space-y-2">
						<Label>มุมมอง</Label>
						<div class="flex gap-2">
							<Button
								variant={viewKind === 'learning_group' ? 'default' : 'outline'}
								onclick={() => changeViewKind('learning_group')}>กลุ่มเรียน</Button
							>
							<Button
								variant={viewKind === 'homeroom' ? 'default' : 'outline'}
								onclick={() => changeViewKind('homeroom')}>ห้องประจำชั้น</Button
							>
						</div>
					</div>
					<div class="space-y-2">
						<Label for="timetable-target"
							>{viewKind === 'learning_group' ? 'กลุ่มเรียน' : 'ห้องประจำชั้น'}</Label
						>
						<select
							id="timetable-target"
							class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
							value={selectedTargetId}
							onchange={changeTarget}
						>
							{#if viewKind === 'learning_group'}
								{#each groups as group (group.id)}<option value={group.id}
										>{groupLabel(group)}</option
									>{/each}
							{:else}
								{#each homerooms as homeroom (homeroom.id)}<option value={homeroom.id}
										>{homeroom.code} · {homeroom.name}</option
									>{/each}
							{/if}
						</select>
					</div>
				</Card.Content>
			</Card.Root>

			{#if dirty}
				<p class="text-amber-700 dark:text-amber-300 text-sm">
					มีแบบร่างคาบที่ยังไม่บันทึก — ต้องบันทึกหรือยกเลิกก่อนเปลี่ยนบริบท
				</p>
			{/if}
			{#if errorMessage}
				<div
					class="border-destructive/30 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm"
				>
					{errorMessage}
				</div>
			{/if}

			<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1.7fr)_22rem]">
				<Card.Root class="overflow-hidden">
					<Card.Header>
						<Card.Title class="flex items-center gap-2"
							><CalendarClock /> ตารางรายสัปดาห์</Card.Title
						>
						<Card.Description>คลิกช่องว่างเพื่อเพิ่มคาบ หรือคลิกคาบเดิมเพื่อแก้ไข</Card.Description>
					</Card.Header>
					<Card.Content class="overflow-x-auto">
						<table class="w-full min-w-[760px] border-separate border-spacing-0 text-sm">
							<thead>
								<tr>
									<th class="bg-muted/70 sticky left-0 z-10 border p-2 text-left">คาบ</th>
									{#each activeDays as day (day.value)}<th
											class="bg-muted/70 border p-2 text-center">{day.label}</th
										>{/each}
								</tr>
							</thead>
							<tbody>
								{#each periods as period (period.id)}
									<tr>
										<th
											class="bg-background sticky left-0 z-10 w-28 border p-2 text-left font-normal"
										>
											<p class="font-medium">{period.name ?? `คาบ ${period.orderIndex}`}</p>
											<p class="text-muted-foreground text-xs">
												{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
											</p>
										</th>
										{#each activeDays as day (day.value)}
											{@const cellEntries = entriesForCell(day.value, period.id)}
											<td class="h-24 min-w-32 border p-1 align-top">
												{#if cellEntries.length === 0}
													<button
														type="button"
														class="hover:bg-muted text-muted-foreground flex size-full min-h-20 items-center justify-center rounded-md border border-dashed"
														disabled={!canManage || !selectedTargetId}
														onclick={() => startAtCell(day.value, period.id)}
														title="เพิ่มคาบ"><Plus class="size-4" /></button
													>
												{:else}
													<div class="space-y-1">
														{#each cellEntries as entry (entry.id)}
															<button
																type="button"
																class="bg-primary/10 hover:bg-primary/15 border-primary/20 w-full rounded-md border p-2 text-left {selectedEntryId ===
																entry.id
																	? 'ring-primary ring-2'
																	: ''}"
																onclick={() => editEntry(entry)}
															>
																<p class="truncate font-medium">{entryTitle(entry)}</p>
																<p class="text-muted-foreground mt-1 truncate text-xs">
																	{entry.roomCode ??
																		entry.learningGroupCode ??
																		entry.homeroomName ??
																		'ไม่ระบุห้อง'}
																</p>
															</button>
														{/each}
													</div>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</Card.Content>
				</Card.Root>

				<Card.Root>
					<Card.Header>
						<div class="flex items-center justify-between gap-2">
							<div>
								<Card.Title>{selectedEntry ? 'แก้ไขคาบ' : 'เพิ่มคาบ'}</Card.Title>
								<Card.Description
									>{selectedEntry
										? `เวอร์ชัน ${selectedEntry.rowVersion}`
										: 'เลือกวันและคาบที่ต้องการ'}</Card.Description
								>
							</div>
							{#if dirty}<Badge variant="secondary">ยังไม่บันทึก</Badge>{/if}
						</div>
					</Card.Header>
					<Card.Content class="space-y-4">
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-2">
								<Label for="entry-day">วัน</Label>
								<select
									id="entry-day"
									class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
									bind:value={formDay}
									disabled={!canManage}
									onchange={markDirty}
									>{#each activeDays as day (day.value)}<option value={day.value}
											>{day.label}</option
										>{/each}</select
								>
							</div>
							<div class="space-y-2">
								<Label for="entry-period">คาบ</Label>
								<select
									id="entry-period"
									class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
									bind:value={formPeriodId}
									disabled={!canManage}
									onchange={markDirty}
									>{#each periods as period (period.id)}<option value={period.id}
											>{period.name ?? period.orderIndex}</option
										>{/each}</select
								>
							</div>
						</div>
						<div class="space-y-2">
							<Label for="entry-type">ประเภทคาบ</Label>
							<select
								id="entry-type"
								class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
								bind:value={formEntryType}
								disabled={!canManage || Boolean(selectedEntry)}
								onchange={markDirty}
								>{#each entryTypeOptions as option (option.value)}<option value={option.value}
										>{option.label}</option
									>{/each}</select
							>
						</div>
						<div class="space-y-2">
							<Label for="entry-room">ห้องเรียน</Label>
							<select
								id="entry-room"
								class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
								bind:value={formRoomId}
								disabled={!canManage}
								onchange={markDirty}
							>
								<option value="">ไม่ระบุ</option>
								{#each rooms as room (room.id)}<option value={room.id}
										>{room.code ? `${room.code} · ` : ''}{room.name_th}</option
									>{/each}
							</select>
						</div>
						<div class="space-y-2">
							<Label for="entry-title">ชื่อแสดงเพิ่มเติม</Label>
							<Input
								id="entry-title"
								bind:value={formTitle}
								disabled={!canManage}
								oninput={markDirty}
							/>
						</div>
						<div class="space-y-2">
							<Label for="entry-note">หมายเหตุ</Label>
							<Input
								id="entry-note"
								bind:value={formNote}
								disabled={!canManage}
								oninput={markDirty}
							/>
						</div>
						{#if canManage}
							<div class="flex flex-wrap gap-2 pt-2">
								<Button
									disabled={busy || !dirty || !selectedTargetId || !formPeriodId}
									onclick={saveEntry}
									>{#if busy}<Loader2 class="animate-spin" />{:else}<Save />{/if} บันทึก</Button
								>
								<Button variant="outline" disabled={busy || !dirty} onclick={resetForm}
									><RotateCcw /> ยกเลิก</Button
								>
								{#if selectedEntry}<Button
										variant="destructive"
										class="ml-auto"
										disabled={busy}
										onclick={removeEntry}><Trash2 /> ลบคาบ</Button
									>{/if}
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</PageShell>
