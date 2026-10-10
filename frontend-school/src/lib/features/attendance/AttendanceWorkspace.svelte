<script lang="ts">
	import { CalendarDays, Users, Info, RefreshCw, Search, Check } from '@lucide/svelte';
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
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
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
		onDateChange
	}: {
		term: string;
		date: string;
		initial: AttendanceWorkspace;
		onDateChange?: (date: string) => void;
	} = $props();
	let workspace = $state(untrack(() => initial)),
		selectedDate = $state(untrack(() => date)),
		kind = $state<AttendanceKind>('flag'),
		detail = $state<AttendanceDetail | null>(null),
		busy = $state(false),
		pendingAction = $state(''),
		workspaceError = $state(''),
		detailError = $state(''),
		mutationError = $state(''),
		confirmCancellation = $state(false),
		failedSession = $state.raw<AttendanceSession | null>(null),
		reason = $state(''),
		search = $state('');
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
	const request = new LatestRequest();
	onDestroy(() => request.abort());
	const students = $derived(detail?.students.filter((s) => s.displayName.includes(search)) ?? []);
	async function refresh() {
		const t = request.begin();
		busy = true;
		pendingAction = 'workspace';
		workspaceError = '';
		detailError = '';
		if (selectedDate !== workspace.date) {
			detail = null;
			mutationError = '';
			reason = '';
		}
		try {
			const result = await attendanceWorkspace(term, selectedDate, { signal: t.signal });
			if (request.isCurrent(t.revision)) workspace = result;
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
		if (busy || (s.rowVersion === 0 && !mayWrite(s))) return;
		const t = request.begin();
		busy = true;
		pendingAction = 'detail';
		detail = null;
		detailError = '';
		failedSession = s;
		mutationError = '';
		try {
			const loaded =
				s.rowVersion === 0
					? await openAttendanceSession({
							academicTermId: term,
							date: workspace.date,
							kind: s.kind,
							sourceKey: s.sourceKey
						})
					: await attendanceDetail(s.id, { signal: t.signal });
			if (!request.isCurrent(t.revision)) return;
			detail = loaded;
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
		if (!detail || busy || !detail.writable || !canWrite) return;
		mutationError = '';
		const records = detail.students
			.filter((s) => s.result !== 'unchecked')
			.map((s) => ({ studentId: s.studentId, result: s.result, note: s.note }));
		if (!records.length) {
			mutationError = 'เลือกผลอย่างน้อยหนึ่งคน หรือใช้ปุ่มขาดทั้งหมด';
			return;
		}
		if (detail.session.savedAt && !reason.trim()) {
			mutationError = 'ระบุเหตุผลเมื่อแก้ไขผลที่บันทึกแล้ว';
			return;
		}
		const t = request.begin();
		busy = true;
		pendingAction = 'save';
		try {
			const saved = await saveAttendanceResults(detail.session.id, {
				rowVersion: detail.session.rowVersion,
				reason,
				students: records
			});
			if (!request.isCurrent(t.revision)) return;
			detail = saved;
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((s) => (s.id === detail?.session.id ? detail.session : s))
			};
			toast.success('บันทึกแล้ว');
			reason = '';
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
				rowVersion: detail.session.rowVersion,
				cancelled: !detail.session.cancelled,
				reason
			});
			if (!request.isCurrent(t.revision)) return;
			detail = saved;
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

<Card
	class="gap-0 overflow-hidden border-primary/20 bg-gradient-to-br from-primary/10 via-card to-card py-0"
>
	<CardContent class="flex flex-col gap-4 p-4 sm:flex-row sm:items-end sm:justify-between sm:p-5">
		<div class="space-y-2">
			<p class="flex items-center gap-2 text-sm font-medium text-primary">
				<CalendarDays class="size-4" />เช็คชื่อประจำวัน
			</p>
			<h2 class="text-xl font-semibold">{formatCalendarDate(selectedDate)}</h2>
			<p class="text-sm text-muted-foreground">
				เลือกวันที่และประเภทรอบ แล้วเปิดรายชื่อนักเรียนที่ต้องเช็คชื่อ
			</p>
		</div>
		<div class="flex flex-wrap items-end gap-3">
			<div class="space-y-2">
				<Label for="attendance-date">วันที่</Label><DatePicker
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
			<LoadingButton
				type="button"
				loading={pendingAction === 'workspace'}
				variant="outline"
				onclick={refresh}
				disabled={busy}><RefreshCw class="size-4" />โหลดผลล่าสุด</LoadingButton
			>
		</div>
	</CardContent>
</Card>
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
					detail = null;
					detailError = '';
					mutationError = '';
					reason = '';
				}}>{label}</Button
			>
		{/each}
	</div>
	<span class="text-sm text-muted-foreground" role="status"
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
<div class="grid items-start gap-6 xl:grid-cols-[minmax(240px,320px)_minmax(0,1fr)]">
	<Card aria-busy={pendingAction === 'workspace'}>
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
						disabled={busy || (s.rowVersion === 0 && !mayWrite(s))}
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
	</Card>
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
			<CardHeader class="border-b">
				<div class="flex flex-wrap items-start justify-between gap-3">
					<div class="min-w-0 space-y-2">
						<CardTitle><h2 class="break-words text-lg">{detail.session.title}</h2></CardTitle
						><CardDescription class="flex items-center gap-2"
							><Users class="size-4" />นักเรียน {detail.students.length} คน · {detail.counted
								? 'นับในสรุป'
								: 'ไม่นับในสรุป'}</CardDescription
						>
					</div>
					<Badge variant="secondary"
						>{detail.session.cancelled
							? 'งดเช็คชื่อ'
							: detail.session.savedAt
								? 'บันทึกแล้ว'
								: 'ยังไม่บันทึก'}</Badge
					>
				</div>
			</CardHeader>
			<CardContent>
				<form
					class="space-y-5"
					aria-label="บันทึกผลเช็คชื่อ"
					onsubmit={(event) => {
						event.preventDefault();
						void save();
					}}
				>
					{#if !detail.session.savedAt}<Alert
							><Info /><AlertTitle>บันทึกเมื่อเช็คชื่อพร้อมแล้ว</AlertTitle><AlertDescription
								>กดบันทึกแล้วคนที่ยังไม่เช็คจะเป็นขาด ครูต้องระบุอย่างน้อยหนึ่งคนก่อนบันทึก
								หากไม่มีใครมาให้เลือกขาดทั้งหมด</AlertDescription
							></Alert
						>{/if}
					<div class="flex flex-wrap items-end justify-between gap-3">
						<div class="w-full space-y-2 sm:w-64">
							<Label for="attendance-search"><Search class="size-4" />ค้นหาชื่อนักเรียน</Label
							><Input
								id="attendance-search"
								placeholder="ค้นหาชื่อนักเรียน"
								bind:value={search}
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
							</div>{/if}
					</div>
					<Table.Root class="min-w-[640px]">
						<Table.Header
							><Table.Row
								><Table.Head class="w-56">เลขที่ / ชื่อ</Table.Head><Table.Head class="w-40"
									>ผล</Table.Head
								><Table.Head class="w-32">เข้าโรงเรียน</Table.Head><Table.Head class="min-w-48"
									>หมายเหตุ</Table.Head
								></Table.Row
							></Table.Header
						>
						<Table.Body
							>{#each students as s (s.studentId)}<Table.Row
									><Table.Cell
										><span class="text-muted-foreground">{s.classNumber ?? '-'} · </span><span
											class="font-medium">{s.displayName}</span
										></Table.Cell
									><Table.Cell
										><AttendanceSelect
											label={`ผลของ ${s.displayName}`}
											value={s.result}
											onValueChange={(value) => {
												s.result = value as AttendanceResult;
											}}
											disabled={!detail.writable || !canWrite || busy}
											options={Object.entries(resultLabels)
												.filter(([value]) => value !== 'unchecked' || !detail?.session.savedAt)
												.map(([value, label]) => ({ value, label }))}
										/></Table.Cell
									><Table.Cell class="text-muted-foreground"
										>{s.arrivalAt
											? new Date(s.arrivalAt).toLocaleTimeString('th-TH', {
													timeZone: 'Asia/Bangkok'
												})
											: '—'}</Table.Cell
									><Table.Cell
										><Input
											aria-label={`หมายเหตุของ ${s.displayName}`}
											bind:value={s.note}
											maxlength={1000}
											disabled={!detail.writable || !canWrite || busy}
										/></Table.Cell
									></Table.Row
								>{:else}<Table.Row
									><Table.Cell colspan={4} class="py-8 text-center text-muted-foreground"
										>{search ? 'ไม่พบชื่อนักเรียนที่ค้นหา' : 'ไม่มีนักเรียนในรอบนี้'}</Table.Cell
									></Table.Row
								>{/each}</Table.Body
						>
					</Table.Root>
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
							><LoadingButton
								type="submit"
								loading={pendingAction === 'save'}
								disabled={busy || !detail.writable}>บันทึก</LoadingButton
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
