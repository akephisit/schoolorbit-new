<script lang="ts">
	import AttendanceSelect from './AttendanceSelect.svelte';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { ATTENDANCE_FACE_PERMISSIONS } from './attendance-access.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { can } from '#lib/stores/permissions.js';
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
	let { term, date, initial }: { term: string; date: string; initial: AttendanceWorkspace } =
		$props();
	let workspace = $state(untrack(() => initial)),
		selectedDate = $state(untrack(() => date)),
		kind = $state<AttendanceKind>('flag'),
		detail = $state<AttendanceDetail | null>(null),
		busy = $state(false),
		pendingAction = $state(''),
		workspaceError = $state(''),
		detailError = $state(''),
		failedSession = $state.raw<AttendanceSession | null>(null),
		reason = $state(''),
		search = $state('');
	const canWrite = $derived(
		$can.hasAny(
			PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED,
			PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL,
			PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL
		)
	);
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
		detail = null;
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
		if (busy || (s.rowVersion === 0 && !canWrite)) return;
		const t = request.begin();
		busy = true;
		pendingAction = 'detail';
		detail = null;
		detailError = '';
		failedSession = s;
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
		if (detail) detail = { ...detail, students: detail.students.map((s) => ({ ...s, result })) };
	}
	async function save() {
		if (!detail || busy) return;
		const records = detail.students
			.filter((s) => s.result !== 'unchecked')
			.map((s) => ({ studentId: s.studentId, result: s.result, note: s.note }));
		if (!records.length) {
			toast.error('เลือกผลอย่างน้อยหนึ่งคน หรือใช้ปุ่มขาดทั้งหมด');
			return;
		}
		busy = true;
		pendingAction = 'save';
		try {
			detail = await saveAttendanceResults(detail.session.id, {
				rowVersion: detail.session.rowVersion,
				reason,
				students: records
			});
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((s) => (s.id === detail?.session.id ? detail.session : s))
			};
			toast.success('บันทึกแล้ว');
			reason = '';
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่ได้');
		} finally {
			busy = false;
			pendingAction = '';
		}
	}
	async function cancel() {
		if (!detail || busy) return;
		if (!reason.trim()) {
			toast.error('ระบุเหตุผลงดหรือคืนคาบ');
			return;
		}
		busy = true;
		pendingAction = 'cancel';
		try {
			detail = await cancelAttendance(detail.session.id, {
				rowVersion: detail.session.rowVersion,
				cancelled: !detail.session.cancelled,
				reason
			});
			workspace = {
				...workspace,
				sessions: workspace.sessions.map((s) => (s.id === detail?.session.id ? detail.session : s))
			};
			reason = '';
			toast.success('บันทึกแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่ได้');
		} finally {
			busy = false;
			pendingAction = '';
		}
	}
</script>

<nav class="flex flex-wrap gap-3">
	{#if $can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL)}<Button
			variant="outline"
			href={`/staff/attendance/settings?academicTermId=${term}`}>ตั้งค่าปฏิทินและรอบพิเศษ</Button
		>{/if}{#if $can.hasAny(...ATTENDANCE_FACE_PERMISSIONS)}<Button
			variant="outline"
			href={`/staff/attendance/faces?academicTermId=${term}`}>เว็บแคม / ลงทะเบียนใบหน้า</Button
		>{/if}<Button variant="outline" href={`/staff/attendance/report?academicTermId=${term}`}
		>สรุป / ล้างภาคเรียน</Button
	>
</nav>
<div class="flex flex-wrap items-center gap-3">
	<label
		>วันที่ <DatePicker
			bind:value={selectedDate}
			onValueChange={() => {
				void refresh();
			}}
			ariaLabel="วันที่"
			disabled={busy}
		/></label
	><LoadingButton
		loading={pendingAction === 'workspace'}
		variant="outline"
		onclick={refresh}
		disabled={busy}>โหลดผลล่าสุด</LoadingButton
	><span class="text-sm text-muted-foreground"
		>{selectedDate !== workspace.date
			? 'กำลังโหลดวันที่เลือก'
			: workspace.counted
				? 'นับในสรุป'
				: 'บันทึกได้ แต่วันนี้ไม่นับในสรุป'}</span
	>
</div>
<div class="flex flex-wrap gap-2">
	{#each Object.entries(kindLabels) as [value, label] (value)}
		<Button
			variant={kind === value ? 'default' : 'outline'}
			disabled={busy}
			onclick={() => {
				kind = value as AttendanceKind;
				detail = null;
			}}>{label}</Button
		>{/each}
</div>
{#if workspaceError}<PageState
		variant="error"
		title="โหลดวันเช็คชื่อไม่ได้"
		description={workspaceError}
		actionLabel="ลองโหลดวันเช็คชื่อใหม่"
		onaction={refresh}
	/>{/if}
<div class="grid gap-4 lg:grid-cols-[minmax(200px,300px)_1fr]">
	<aside class="space-y-2">
		{#each choices as s (s.id)}
			<Button
				class="h-auto w-full justify-start whitespace-normal py-3 text-left"
				variant={detail?.session.id === s.id ? 'default' : 'outline'}
				onclick={() => select(s)}
				disabled={busy || (s.rowVersion === 0 && !canWrite)}
				>{s.startTime.slice(0, 5)} · {s.title}{s.cancelled
					? ' · งด'
					: s.savedAt
						? ' · บันทึกแล้ว'
						: ''}</Button
			>{:else}<p class="text-muted-foreground">ไม่มีรอบที่ได้รับมอบหมายในวันนี้</p>{/each}
	</aside>
	<section class="min-w-0 space-y-4" aria-busy={pendingAction === 'detail'}>
		{#if pendingAction === 'detail'}<PageSkeleton
				variant="table"
				rows={4}
				columns={4}
			/>{:else if detailError}<PageState
				variant="error"
				title="เปิดรอบไม่ได้"
				description={detailError}
				actionLabel="ลองเปิดรอบใหม่"
				onaction={() => {
					if (failedSession) void select(failedSession);
				}}
			/>{:else if detail}<h2 class="text-lg font-semibold">{detail.session.title}</h2>
			<p class="text-sm text-muted-foreground">
				{detail.counted ? 'นับในสรุป' : 'ไม่นับในสรุป'} · นักเรียน {detail.students.length} คน
			</p>
			{#if !detail.session.savedAt}<p class="rounded-lg bg-muted p-3 text-sm">
					กดบันทึกแล้วคนที่ยังไม่เช็คจะเป็นขาด ครูต้องระบุอย่างน้อยหนึ่งคนก่อนบันทึก
					หากไม่มีใครมาให้เลือกขาดทั้งหมด
				</p>{/if}
			<div class="flex flex-wrap gap-2">
				<Input
					placeholder="ค้นหาชื่อนักเรียน"
					bind:value={search}
					class="max-w-xs"
					aria-label="ค้นหาชื่อนักเรียน"
				/>{#if detail.writable && canWrite}<Button
						variant="outline"
						onclick={() => markAll('present')}
						disabled={busy}>มาทั้งหมด</Button
					><Button variant="outline" onclick={() => markAll('absent')} disabled={busy}
						>ขาดทั้งหมด</Button
					>{/if}
			</div>
			<Table.Root class="min-w-[640px]"
				><Table.Header
					><Table.Row
						><Table.Head>เลขที่ / ชื่อ</Table.Head><Table.Head>ผล</Table.Head><Table.Head
							>เข้าโรงเรียน</Table.Head
						><Table.Head>หมายเหตุ</Table.Head></Table.Row
					></Table.Header
				><Table.Body
					>{#each students as s (s.studentId)}
						<Table.Row
							><Table.Cell>{s.classNumber ?? '-'} · {s.displayName}</Table.Cell><Table.Cell
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
							><Table.Cell
								>{s.arrivalAt
									? new Date(s.arrivalAt).toLocaleTimeString('th-TH', { timeZone: 'Asia/Bangkok' })
									: '—'}</Table.Cell
							><Table.Cell
								><Input
									aria-label={`หมายเหตุของ ${s.displayName}`}
									bind:value={s.note}
									disabled={!detail.writable || !canWrite || busy}
								/></Table.Cell
							></Table.Row
						>{/each}</Table.Body
				></Table.Root
			>
			{#if canWrite}<label class="block"
					>เหตุผลแก้ไข / งดคาบ<Input
						bind:value={reason}
						maxlength={1000}
						placeholder={detail.session.savedAt
							? 'ระบุเมื่อแก้ไขผลที่บันทึกแล้ว'
							: 'เช่น งดเรียนเพราะกิจกรรมโรงเรียน'}
					/></label
				>
				<div class="flex flex-wrap gap-2">
					<LoadingButton
						loading={pendingAction === 'save'}
						onclick={save}
						disabled={busy || !detail.writable}>บันทึก</LoadingButton
					><Button variant="outline" onclick={cancel} disabled={busy}
						>{detail.session.cancelled ? 'คืนคาบเช็คชื่อ' : 'งดคาบ / กิจกรรมแทนการเรียน'}</Button
					>
				</div>{/if}
		{:else}<PageState
				title="เลือกรอบเช็คชื่อ"
				description={canWrite
					? 'เลือกรอบที่ต้องการจากรายการ'
					: 'ดูผลรอบที่ครูเปิดเช็คชื่อแล้วได้จากรายการ'}
			/>{/if}
	</section>
</div>
