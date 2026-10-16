<script lang="ts">
	import { CalendarDays, Users, Search, Check } from '@lucide/svelte';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Alert, AlertDescription, AlertTitle } from '#lib/components/ui/alert/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import AttendanceSelect from './AttendanceSelect.svelte';
	import AttendanceResultToggle from './AttendanceResultToggle.svelte';
	import AttendanceStatusBadge from './AttendanceStatusBadge.svelte';
	import { lessonRoomOptions, matchesLessonRoom, roundOption } from './round-presentation.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		copyAttendanceDetail,
		mergeAttendanceDetail,
		attendanceResultPatch,
		type AttendanceConflict
	} from './session-merge.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		attendanceWorkspace,
		attendanceSessionContext,
		attendanceDetail,
		openAttendanceSession,
		saveAttendanceResults,
		cancelAttendance,
		kindLabels,
		resultLabels,
		type AttendanceWorkspace,
		type AttendanceDetail,
		type AttendanceSession,
		type AttendanceKind,
		type AttendanceResult
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial,
		initialDetail,
		onDateChange
	}: {
		term: string;
		date: string;
		initial: AttendanceWorkspace;
		initialDetail?: AttendanceDetail;
		onDateChange?: (date: string) => void;
	} = $props();
	let workspace = $state(untrack(() => initial)),
		selectedDate = $state(untrack(() => date)),
		kind = $state<AttendanceKind>(untrack(() => initialDetail?.session.kind ?? 'arrival')),
		detail = $state<AttendanceDetail | null>(untrack(() => initialDetail ?? null)),
		busy = $state(false),
		pendingAction = $state(''),
		workspaceError = $state(''),
		detailError = $state(''),
		mutationError = $state(''),
		confirmCancellation = $state(false),
		failedSession = $state.raw<AttendanceSession | null>(null),
		reason = $state(''),
		search = $state('');
	let baseline = $state.raw<AttendanceDetail | null>(
		untrack(() => (initialDetail ? copyAttendanceDetail(initialDetail) : null))
	);
	let conflicts = $state<AttendanceConflict[]>([]);
	let rosterPage = $state(1);
	let lessonRoom = $state('');
	const rosterPageSize = 40;
	const canStartAny = $derived(
		$can.hasAny(
			PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED,
			PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL,
			PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL
		)
	);
	function mayWrite(session: AttendanceSession) {
		return (
			$can.hasAny(PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL, PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL) ||
			($can.has(PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED) &&
				!!$authStore.user &&
				session.teacherIds.includes($authStore.user.id))
		);
	}
	const canWrite = $derived(detail ? mayWrite(detail.session) : canStartAny);
	const choices = $derived(
		workspaceError || (pendingAction === 'workspace' && selectedDate !== workspace.date)
			? []
			: workspace.sessions.filter((s) => s.kind === kind)
	);
	const lessonChoices = $derived(workspace.lessonChoices);
	const rooms = $derived(lessonRoomOptions(lessonChoices, choices));
	const activeRoom = $derived(rooms.some((room) => room.value === lessonRoom) ? lessonRoom : '');
	const filteredChoices = $derived(
		kind === 'lesson'
			? choices.filter((session) => matchesLessonRoom(session, lessonChoices, activeRoom))
			: choices
	);
	const selectedRound = $derived(detail?.session ?? failedSession);
	const selectedInFilter = $derived(
		selectedRound ? filteredChoices.some((session) => session.id === selectedRound.id) : false
	);
	const request = new LatestRequest();
	onDestroy(() => request.abort());
	const students = $derived(detail?.students.filter((s) => s.displayName.includes(search)) ?? []);
	const visibleStudents = $derived(
		students.slice((rosterPage - 1) * rosterPageSize, rosterPage * rosterPageSize)
	);
	function chooseConflict(conflict: AttendanceConflict, source: 'local' | 'latest') {
		const row = detail?.students.find((row) => row.studentId === conflict.studentId);
		if (!row) return;
		if (conflict.fields.includes('result')) row.result = conflict[source].result;
		conflicts = conflicts.filter((item) => item.studentId !== conflict.studentId);
	}
	async function refresh() {
		if (busy) return;
		const selected = selectedDate === workspace.date ? detail?.session : undefined;
		const t = request.begin();
		busy = true;
		pendingAction = 'workspace';
		failedSession = null;
		conflicts = [];
		baseline = null;
		detail = null;
		rosterPage = 1;
		workspaceError = '';
		detailError = '';
		if (selectedDate !== workspace.date) {
			lessonRoom = '';
			detail = null;
			mutationError = '';
			reason = '';
		}
		try {
			const [result, loaded] = await Promise.all([
				attendanceWorkspace(term, selectedDate, { signal: t.signal }),
				selected
					? selected.rowVersion === 0
						? openAttendanceSession(attendanceSessionContext(selected), { signal: t.signal })
						: attendanceDetail(selected.id, { signal: t.signal })
					: Promise.resolve(null)
			]);
			if (request.isCurrent(t.revision)) {
				workspace = loaded
					? {
							...result,
							sessions: result.sessions.map((row) =>
								row.id === loaded.session.id ? loaded.session : row
							)
						}
					: result;
				detail = loaded;
				baseline = loaded ? copyAttendanceDetail(loaded) : null;
				mutationError = '';
				reason = '';
			}
		} catch (e) {
			if (request.isCurrent(t.revision))
				workspaceError = e instanceof Error ? e.message : 'โหลดไม่ได้';
		} finally {
			if (request.isCurrent(t.revision)) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function select(s: AttendanceSession) {
		if (busy) return;
		const t = request.begin();
		busy = true;
		pendingAction = 'detail';
		detail = null;
		baseline = null;
		conflicts = [];
		rosterPage = 1;
		detailError = '';
		failedSession = s;
		mutationError = '';
		try {
			const loaded =
				s.rowVersion === 0
					? await openAttendanceSession(
							{
								academicTermId: term,
								date: workspace.date,
								kind: s.kind,
								sourceKey: s.sourceKey
							},
							{ signal: t.signal }
						)
					: await attendanceDetail(s.id, { signal: t.signal });
			if (!request.isCurrent(t.revision)) return;
			detail = loaded;
			baseline = copyAttendanceDetail(loaded);
			failedSession = null;
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((row) =>
					row.kind === loaded.session.kind && row.sourceKey === loaded.session.sourceKey
						? loaded.session
						: row
				)
			};
			reason = '';
		} catch (e) {
			if (request.isCurrent(t.revision))
				detailError = e instanceof Error ? e.message : 'เปิดรอบไม่ได้';
		} finally {
			if (request.isCurrent(t.revision)) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	function markAll(result: AttendanceResult) {
		if (detail && detail.writable && canWrite && !busy)
			detail = { ...detail, students: detail.students.map((s) => ({ ...s, result })) };
	}
	async function save() {
		if (!detail || busy || conflicts.length || !detail.writable || !canWrite) return;
		mutationError = '';
		if (!baseline) return;
		const patch = attendanceResultPatch(baseline, detail);
		if (!patch.students.length) {
			mutationError = 'ยังไม่มีผลที่เปลี่ยนแปลงสำหรับบันทึก';
			return;
		}
		if (patch.needsReason && !reason.trim()) {
			mutationError = 'ระบุเหตุผลเมื่อแก้ไขผลที่บันทึกแล้ว';
			return;
		}
		const t = request.begin();
		busy = true;
		pendingAction = 'save';
		try {
			const saved = await saveAttendanceResults(detail.session.id, {
				session: attendanceSessionContext(detail.session),
				rosterToken: detail.session.rosterToken,
				rowVersion: detail.session.rowVersion,
				reason,
				students: patch.students
			});
			if (!request.isCurrent(t.revision)) return;
			detail = saved;
			baseline = copyAttendanceDetail(saved);
			conflicts = [];
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((s) => (s.id === detail?.session.id ? detail.session : s))
			};
			toast.success('บันทึกแล้ว');
			reason = '';
		} catch (e) {
			if (request.isCurrent(t.revision)) {
				mutationError = e instanceof Error ? e.message : 'บันทึกไม่ได้';
				if (e instanceof ApiClientError && e.status === 409 && detail && baseline) {
					try {
						const latest = await attendanceDetail(detail.session.id, { signal: t.signal });
						if (!request.isCurrent(t.revision)) return;
						const merged = mergeAttendanceDetail(baseline, detail, latest);
						detail = merged.detail;
						baseline = copyAttendanceDetail(latest);
						conflicts = merged.conflicts;
						workspace = {
							...workspace,
							sessions: workspace.sessions.map((row) =>
								row.id === latest.session.id ? latest.session : row
							)
						};
					} catch (error) {
						if (request.isCurrent(t.revision))
							mutationError +=
								' · ' + (error instanceof Error ? error.message : 'โหลดผลล่าสุดไม่ได้');
					}
				}
			}
		} finally {
			if (request.isCurrent(t.revision)) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function cancel() {
		if (!detail || busy || !canWrite) return;
		mutationError = '';
		if (!reason.trim()) {
			mutationError = 'ระบุเหตุผลงดหรือคืนคาบ';
			return;
		}
		const t = request.begin();
		busy = true;
		pendingAction = 'cancel';
		try {
			const saved = await cancelAttendance(detail.session.id, {
				session: attendanceSessionContext(detail.session),
				rosterToken: detail.session.rosterToken,
				rowVersion: detail.session.rowVersion,
				cancelled: !detail.session.cancelled,
				reason
			});
			if (!request.isCurrent(t.revision)) return;
			detail = saved;
			baseline = copyAttendanceDetail(saved);
			conflicts = [];
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((s) => (s.id === detail?.session.id ? detail.session : s))
			};
			reason = '';
			toast.success('บันทึกแล้ว');
		} catch (e) {
			if (request.isCurrent(t.revision))
				mutationError = e instanceof Error ? e.message : 'บันทึกไม่ได้';
		} finally {
			if (request.isCurrent(t.revision)) {
				busy = false;
				pendingAction = '';
			}
		}
	}
</script>

<section class="rounded-xl border bg-card p-3 sm:p-4" aria-label="วันที่เช็คชื่อ">
	<div class="flex flex-wrap items-end gap-3">
		<div class="min-w-0 flex-1 space-y-2">
			<Label for="attendance-date">วันที่เช็คชื่อ</Label>
			<DatePicker
				id="attendance-date"
				bind:value={selectedDate}
				onValueChange={(value) => {
					if (value) onDateChange?.(value);
					void refresh();
				}}
				ariaLabel="วันที่"
				disabled={busy}
			/>
		</div>
	</div>
</section>
<div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
	<div class="flex flex-wrap gap-2" role="group" aria-label="ประเภทรอบเช็คชื่อ">
		{#each Object.entries(kindLabels) as [value, label] (value)}
			<Button
				type="button"
				variant={kind === value ? 'secondary' : 'outline'}
				aria-pressed={kind === value}
				disabled={busy}
				onclick={() => {
					kind = value as AttendanceKind;
					baseline = null;
					conflicts = [];
					detail = null;
					detailError = '';
					mutationError = '';
					reason = '';
					failedSession = null;
				}}>{label}</Button
			>
		{/each}
	</div>
	<span class="sr-only" role="status"
		>{selectedDate !== workspace.date
			? workspaceError
				? 'ยังโหลดวันที่เลือกไม่สำเร็จ'
				: 'กำลังโหลดวันที่เลือก'
			: workspace.counted
				? 'นับในสรุป'
				: 'บันทึกได้ แต่วันนี้ไม่นับในสรุป'}</span
	>
</div>
{#if workspaceError}<PageState
		variant="error"
		title="โหลดวันเช็คชื่อไม่ได้"
		description={workspaceError}
		actionLabel="ลองโหลดวันเช็คชื่อใหม่"
		onaction={refresh}
	/>{/if}
<div
	class={kind === 'lesson'
		? 'grid gap-3 sm:grid-cols-[minmax(160px,240px)_minmax(0,1fr)]'
		: 'space-y-2 xl:hidden'}
	aria-busy={pendingAction === 'workspace'}
>
	{#if kind === 'lesson'}
		<div class="space-y-2">
			<Label for="attendance-lesson-room">ห้องเรียน</Label>
			<AttendanceSelect
				id="attendance-lesson-room"
				label="กรองห้องเรียน"
				value={activeRoom || 'all'}
				options={[{ value: 'all', label: 'ทุกห้อง' }, ...rooms]}
				disabled={busy}
				onValueChange={(value) => {
					lessonRoom = value === 'all' ? '' : value;
				}}
			/>
		</div>
	{/if}
	<div class="min-w-0 space-y-2">
		<Label for="attendance-round">รอบ{kindLabels[kind]}</Label>
		<AttendanceSelect
			id="attendance-round"
			label="รอบเช็คชื่อ"
			value={selectedInFilter ? (selectedRound?.id ?? '') : ''}
			placeholder={filteredChoices.length
				? `เลือกรอบ · ${filteredChoices.length} รอบ`
				: choices.length
					? 'ไม่มีคาบในห้องที่เลือก'
					: 'ไม่มีรอบในวันนี้'}
			options={filteredChoices.map((session) => roundOption(session, lessonChoices))}
			disabled={busy || !filteredChoices.length}
			onValueChange={(value) => {
				const selected = filteredChoices.find((s) => s.id === value);
				if (selected) void select(selected);
			}}
		/>
	</div>
</div>
<div
	class={kind === 'lesson'
		? 'min-w-0'
		: 'grid items-start gap-3 xl:grid-cols-[minmax(240px,320px)_minmax(0,1fr)] xl:gap-6'}
>
	{#if kind !== 'lesson'}<Card class="hidden xl:flex" aria-busy={pendingAction === 'workspace'}>
			<CardHeader
				><CardTitle
					><h2 class="flex items-center gap-2">
						<CalendarDays class="size-4 text-primary" />รอบ{kindLabels[kind]}
					</h2></CardTitle
				><CardDescription>เลือกรอบที่ได้รับมอบหมายเพื่อดูหรือบันทึกผล</CardDescription></CardHeader
			>
			<CardContent class="space-y-2">
				{#if pendingAction === 'workspace' && selectedDate !== workspace.date}<PageSkeleton
						variant="detail"
						rows={3}
					/>
				{:else if workspaceError}<p class="text-sm text-muted-foreground">
						โหลดวันที่เลือกอีกครั้งเพื่อดูรอบเช็คชื่อ
					</p>
				{:else}{#each choices as s (s.id)}
						<Button
							type="button"
							class={`h-auto w-full justify-start whitespace-normal py-3 text-left ${detail?.session.id === s.id ? 'border-primary/40 bg-accent' : ''}`}
							variant="outline"
							aria-pressed={detail?.session.id === s.id}
							onclick={() => select(s)}
							disabled={busy}
						>
							<span class="flex w-full min-w-0 flex-col gap-1"
								><span class="font-medium">{s.startTime.slice(0, 5)} · {s.title}</span><span
									class="text-xs text-muted-foreground"
									>{s.cancelled ? 'งด' : s.savedAt ? 'บันทึกแล้ว' : 'ยังไม่บันทึก'}</span
								></span
							>
						</Button>
					{:else}<PageState
							title="ไม่มีรอบในวันนี้"
							description="ไม่มีรอบที่ได้รับมอบหมายในวันนี้ ลองเลือกวันหรือประเภทรอบอื่น"
						/>{/each}{/if}
			</CardContent>
		</Card>{/if}
	<Card
		class="min-w-0"
		aria-busy={pendingAction === 'detail' || pendingAction === 'save' || pendingAction === 'cancel'}
	>
		{#if pendingAction === 'detail'}<CardContent
				><PageSkeleton variant="table" rows={4} columns={4} /></CardContent
			>
		{:else if detailError}<CardContent
				><PageState
					variant="error"
					title="เปิดรอบไม่ได้"
					description={detailError}
					actionLabel="ลองเปิดรอบใหม่"
					onaction={() => {
						if (failedSession) void select(failedSession);
					}}
				/></CardContent
			>
		{:else if detail}
			<CardHeader class="border-b px-3 sm:px-6">
				<div class="flex flex-wrap items-start justify-between gap-3">
					<div class="min-w-0 space-y-2">
						<CardTitle
							><h2 class="break-words text-lg">
								{kind === 'lesson'
									? roundOption(detail.session, lessonChoices).label
									: detail.session.title}
							</h2></CardTitle
						>{#if kind === 'lesson' && roundOption(detail.session, lessonChoices).description}<p
								class="text-sm text-muted-foreground"
							>
								{roundOption(detail.session, lessonChoices).description}
							</p>{/if}<CardDescription class="flex items-center gap-2"
							><Users class="size-4" />นักเรียน {detail.students.length} คน · {detail.counted
								? 'นับในสรุป'
								: 'ไม่นับในสรุป'}</CardDescription
						>
					</div>
					{#if kind === 'lesson' && !selectedInFilter}<Badge variant="outline"
							>รอบนี้อยู่นอกห้องที่กรอง</Badge
						>{/if}
					<Badge variant="secondary"
						>{detail.session.cancelled
							? 'งดเช็คชื่อ'
							: detail.session.savedAt
								? 'บันทึกแล้ว'
								: 'ยังไม่บันทึก'}</Badge
					>
				</div>
			</CardHeader>
			<CardContent class="px-3 sm:px-6">
				<form
					class="space-y-5"
					aria-label="บันทึกผลเช็คชื่อ"
					onsubmit={(event) => {
						event.preventDefault();
						void save();
					}}
				>
					<div class="flex flex-wrap items-end justify-between gap-3">
						<div class="w-full space-y-2 sm:w-64">
							<Label for="attendance-search"><Search class="size-4" />ค้นหาชื่อนักเรียน</Label
							><Input
								id="attendance-search"
								placeholder="ค้นหาชื่อนักเรียน"
								bind:value={search}
								oninput={() => {
									rosterPage = 1;
								}}
								onkeydown={(event) => {
									if (event.key === 'Enter') event.preventDefault();
								}}
							/>
						</div>
						{#if detail.writable && canWrite}<div class="flex flex-wrap gap-2">
								<Button
									type="button"
									variant="outline"
									onclick={() => markAll('present')}
									disabled={busy}><Check class="size-4" />มาทั้งหมด</Button
								><Button
									type="button"
									variant="outline"
									onclick={() => markAll('absent')}
									disabled={busy}>ขาดทั้งหมด</Button
								>
								<LoadingButton
									type="submit"
									loading={pendingAction === 'save'}
									disabled={busy || !detail.writable || conflicts.length > 0}>บันทึก</LoadingButton
								>
							</div>{/if}
					</div>
					<div
						class="max-h-[55svh] overflow-y-auto sm:max-h-none sm:overflow-visible"
						role="region"
						aria-label="รายชื่อนักเรียน"
					>
						<Table.Root class="block min-w-0 sm:table sm:min-w-[720px]">
							<Table.Header class="hidden sm:table-header-group">
								<Table.Row
									><Table.Head class="w-56">เลขที่ / ชื่อ</Table.Head><Table.Head class="w-96"
										>ผล</Table.Head
									><Table.Head class="w-40">มาโรงเรียน</Table.Head></Table.Row
								>
							</Table.Header>
							<Table.Body class="block sm:table-row-group"
								>{#each visibleStudents as s (s.studentId)}<Table.Row
										class="grid grid-cols-1 items-center gap-1 py-1 sm:table-row"
										><Table.Cell
											class="block min-w-0 whitespace-normal px-1 py-0 sm:table-cell sm:p-2"
											><div class="flex items-start justify-between gap-2">
												<span class="min-w-0"
													><span class="text-muted-foreground">{s.classNumber ?? '-'} · </span><span
														class="font-medium">{s.displayName}</span
													></span
												>
												<span class="w-16 shrink-0 text-end text-xs text-muted-foreground"
													>{resultLabels[s.result]}</span
												>
											</div>
											{#if kind !== 'arrival'}<span
													class="flex items-center gap-2 py-1 text-xs sm:hidden"
													>มาโรงเรียน <AttendanceStatusBadge value={s.arrivalResult} /></span
												>{/if}
											{#if s.arrivalAt}<span class="block text-xs text-muted-foreground sm:hidden"
													>เข้าโรงเรียน {new Date(s.arrivalAt).toLocaleTimeString('th-TH', {
														timeZone: 'Asia/Bangkok',
														hour: '2-digit',
														minute: '2-digit'
													})}</span
												>{/if}
										</Table.Cell><Table.Cell class="block p-1 sm:table-cell sm:p-2"
											><AttendanceResultToggle
												label={`ผลของ ${s.displayName}`}
												value={s.result}
												onValueChange={(value) => {
													s.result = value;
												}}
												disabled={!detail.writable || !canWrite || busy}
												allowUnchecked={baseline?.students.find(
													(row) => row.studentId === s.studentId
												)?.result === 'unchecked'}
											/></Table.Cell
										><Table.Cell class="hidden text-muted-foreground sm:table-cell"
											>{#if kind !== 'arrival'}<AttendanceStatusBadge
													value={s.arrivalResult}
												/>{/if}<span class="block text-xs sm:hidden">เข้าโรงเรียน</span>{s.arrivalAt
												? new Date(s.arrivalAt).toLocaleTimeString('th-TH', {
														timeZone: 'Asia/Bangkok'
													})
												: '—'}</Table.Cell
										></Table.Row
									>{:else}<Table.Row
										><Table.Cell colspan={3} class="py-8 text-center text-muted-foreground"
											>{search ? 'ไม่พบชื่อนักเรียนที่ค้นหา' : 'ไม่มีนักเรียนในรอบนี้'}</Table.Cell
										></Table.Row
									>{/each}</Table.Body
							>
						</Table.Root>
					</div>
					<div
						class="flex flex-wrap items-center justify-between gap-3"
						aria-label="หน้ารายชื่อนักเรียน"
					>
						<p class="text-sm text-muted-foreground">
							แสดง {visibleStudents.length} จาก {students.length} คน · คำสั่งทั้งหมดและการบันทึกครอบคลุมทั้งรอบ
						</p>
						<div class="flex gap-2">
							<Button
								type="button"
								variant="outline"
								disabled={rosterPage <= 1 || busy}
								onclick={() => {
									rosterPage--;
								}}>ก่อนหน้า</Button
							><Button
								type="button"
								variant="outline"
								disabled={rosterPage * rosterPageSize >= students.length || busy}
								onclick={() => {
									rosterPage++;
								}}>ถัดไป</Button
							>
						</div>
					</div>
					{#each conflicts as conflict (conflict.studentId)}
						<Alert
							><AlertTitle>ข้อมูลเปลี่ยนพร้อมกัน: {conflict.local.displayName}</AlertTitle
							><AlertDescription>
								<p>ตรวจสอบผลเช็คชื่อก่อนบันทึก</p>
								<p>ของคุณ: {resultLabels[conflict.local.result]}</p>
								<p>ล่าสุด: {resultLabels[conflict.latest.result]}</p>
								<div class="mt-3 flex flex-wrap gap-2">
									<Button
										type="button"
										variant="outline"
										onclick={() => chooseConflict(conflict, 'latest')}
										>ใช้ข้อมูลล่าสุดของ {conflict.local.displayName}</Button
									><Button
										type="button"
										variant="outline"
										onclick={() => chooseConflict(conflict, 'local')}
										>ใช้ข้อมูลของฉันสำหรับ {conflict.local.displayName}</Button
									>
								</div>
							</AlertDescription></Alert
						>
					{/each}

					{#if mutationError}<Alert variant="destructive"
							><AlertTitle>ยังบันทึกไม่สำเร็จ</AlertTitle><AlertDescription
								>{mutationError}</AlertDescription
							></Alert
						>{/if}
					{#if canWrite}<div class="space-y-2">
							<Label for="attendance-reason">เหตุผลแก้ไข / งดคาบ</Label><Input
								id="attendance-reason"
								bind:value={reason}
								maxlength={1000}
								disabled={busy}
								placeholder={detail.session.savedAt
									? 'ระบุเมื่อแก้ไขผลที่บันทึกแล้ว'
									: 'เช่น งดเรียนเพราะกิจกรรมโรงเรียน'}
							/>
						</div>
						<div class="flex flex-wrap items-center justify-end gap-3 border-t pt-4">
							<LoadingButton
								type="button"
								loading={pendingAction === 'cancel'}
								variant="outline"
								onclick={() => {
									if (!reason.trim()) mutationError = 'ระบุเหตุผลงดหรือคืนคาบ';
									else confirmCancellation = true;
								}}
								disabled={busy}
								>{detail.session.cancelled
									? 'คืนคาบเช็คชื่อ'
									: 'งดคาบ / กิจกรรมแทนการเรียน'}</LoadingButton
							>
						</div>
					{:else}<p class="text-sm text-muted-foreground">
							คุณดูผลเช็คชื่อของรอบนี้ได้ การแก้ไขต้องมีสิทธิ์บันทึกผล
						</p>{/if}
				</form>
			</CardContent>
		{:else}<CardContent
				><PageState
					title="เลือกรอบเช็คชื่อ"
					description={canWrite
						? 'เลือกรอบที่ต้องการจากรายการเพื่อเริ่มเช็คชื่อ'
						: 'ดูผลรอบที่ครูเปิดเช็คชื่อแล้วได้จากรายการ'}
				/></CardContent
			>{/if}
	</Card>
</div>
<AlertDialog.Root bind:open={confirmCancellation}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title
				>{detail?.session.cancelled
					? 'คืนคาบเช็คชื่อหรือไม่?'
					: 'ยืนยันงดคาบเช็คชื่อ'}</AlertDialog.Title
			><AlertDialog.Description
				>รอบ {detail?.session.title} · {formatCalendarDate(workspace.date)} การงดคาบจะไม่นับรอบนี้เป็นการขาดเรียน
				เหตุผล: {reason}</AlertDialog.Description
			></AlertDialog.Header
		><AlertDialog.Footer
			><AlertDialog.Cancel>กลับไปตรวจสอบ</AlertDialog.Cancel><AlertDialog.Action
				onclick={cancel}
				disabled={busy || !canWrite}>ยืนยัน</AlertDialog.Action
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
