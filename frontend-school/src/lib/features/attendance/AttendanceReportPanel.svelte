<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { downloadFile } from '#lib/api/files.js';
	import {
		attendanceReport,
		attendanceHistory,
		attendancePurgeImpact,
		purgeAttendanceTerm,
		kindLabels,
		resultLabels,
		type AttendanceReport,
		type AttendanceHistoryItem,
		type AttendancePurgeImpact,
		type AttendanceSummary
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial,
		studentId
	}: { term: string; date: string; initial: AttendanceReport; studentId?: string } = $props();
	let report = $state(untrack(() => initial)),
		history = $state<AttendanceHistoryItem[]>([]),
		selectedStudent = $state(untrack(() => studentId ?? '')),
		busy = $state(false),
		start = $state(untrack(() => date.slice(0, 7)) + '-01'),
		end = $state(untrack(() => date)),
		impact = $state<AttendancePurgeImpact | null>(null),
		reason = $state(''),
		confirmed = $state(false),
		image = $state(''),
		search = $state('');
	let imageGeneration = 0;
	const labels: Record<string, string> = {
		school: 'มาโรงเรียน (วัน)',
		flag: 'หน้าเสาธง (ครั้ง)',
		lesson: 'รายวิชา (คาบ)',
		special: 'รอบพิเศษ (ครั้ง)'
	};
	const summaries = $derived(
		report.summaries.filter(
			(s) => !search || s.displayName.includes(search) || s.scopeLabel.includes(search)
		)
	);
	const canPurge = $derived($can.has(PERMISSIONS.ATTENDANCE_DELETE_SCHOOL) && !studentId);
	function percent(s: AttendanceSummary) {
		const known = s.present + s.late + s.absent + s.leave + s.activity;
		if (!known) return '—';
		return (
			(
				((s.present + s.late + (report.activityCountsAsPresent ? s.activity : 0)) * 100) /
				known
			).toFixed(1) + '%'
		);
	}
	async function run(action: () => Promise<void>) {
		busy = true;
		try {
			await action();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}
	async function loadHistory(id = selectedStudent) {
		if (!id) return;
		await run(async () => {
			history = await attendanceHistory(term, id, start, end);
			selectedStudent = id;
		});
	}
	async function preview(s: AttendanceHistoryItem) {
		if (!s.evidenceFileId) return;
		const token = ++imageGeneration;
		await run(async () => {
			const blob = await downloadFile(s.evidenceFileId!, selectedStudent);
			if (token !== imageGeneration) return;
			if (image) URL.revokeObjectURL(image);
			image = URL.createObjectURL(blob);
		});
	}
	function closeImage() {
		imageGeneration++;
		if (image) URL.revokeObjectURL(image);
		image = '';
	}
	onDestroy(closeImage);
	async function checkPurge() {
		await run(async () => {
			impact = await attendancePurgeImpact(term);
			confirmed = false;
		});
	}
	async function purge() {
		if (!impact || !confirmed || !reason) return;
		await run(async () => {
			impact = await purgeAttendanceTerm(term, { expectedRecords: impact!.records, reason });
			report = await attendanceReport(term);
			history = [];
			closeImage();
			toast.success('ล้างรายละเอียดแล้ว ยอดสรุปยังอยู่');
			confirmed = false;
		});
	}
	function exportCsv() {
		const header = [
			'นักเรียน',
			'ประเภท',
			'รายวิชา/รอบ',
			'มา',
			'สาย',
			'ขาด',
			'ลา',
			'กิจกรรม',
			'ยังไม่เช็ค',
			'ทั้งหมด'
		];
		const rows = report.summaries.map((s) => [
			s.displayName,
			labels[s.category],
			s.scopeLabel,
			s.present,
			s.late,
			s.absent,
			s.leave,
			s.activity,
			s.unchecked,
			s.expected
		]);
		const csv = [header, ...rows]
			.map((row) =>
				row
					.map((value) => {
						const text = String(value);
						const cell =
							typeof value === 'string' && /^[=+@\-\t\r]/u.test(text) ? "'" + text : text;
						return '"' + cell.replaceAll('"', '""') + '"';
					})
					.join(',')
			)
			.join('\r\n');
		const url = URL.createObjectURL(new Blob(['\uFEFF' + csv], { type: 'text/csv;charset=utf-8' }));
		const a = document.createElement('a');
		a.href = url;
		a.download = 'attendance-summary.csv';
		a.click();
		URL.revokeObjectURL(url);
	}
</script>

{#if !studentId}<Button variant="outline" href={`/staff/attendance?academicTermId=${term}`}
		>กลับหน้าเช็คชื่อ</Button
	>{/if}
<p class="rounded-lg bg-muted p-4 text-sm">
	ยอดมาโรงเรียน หน้าเสาธง รายคาบ และรอบพิเศษแยกกัน วันที่ไม่ประมวลผลและคาบที่งดไม่นับขาด
	“ยังไม่เช็ค” แสดงข้อมูลที่ครูยังไม่ได้บันทึก ร้อยละคำนวณเฉพาะผลที่ทราบแล้ว{report.archived
		? ' · ภาคเรียนนี้ล้างรายละเอียดแล้ว แต่ยังเก็บสรุปไว้'
		: ''}
</p>
<div class="flex gap-3">
	<Input aria-label="ค้นหาสรุป" bind:value={search} placeholder="ค้นหารายวิชา / รอบ" /><Button
		variant="outline"
		onclick={exportCsv}>ส่งออก CSV</Button
	>
</div>
<Table.Root
	><Table.Header
		><Table.Row
			>{#each ['นักเรียน', 'ประเภท / รอบ', 'มา', 'สาย', 'ขาด', 'ลา', 'กิจกรรม', 'ยังไม่เช็ค', 'ทั้งหมด', 'ร้อยละ'] as label (label)}
				<Table.Head>{label}</Table.Head>{/each}</Table.Row
		></Table.Header
	><Table.Body
		>{#each summaries as s (s.studentId + s.category + s.scopeKey)}
			<Table.Row
				><Table.Cell
					><Button
						variant="link"
						onclick={() => loadHistory(s.studentId)}
						disabled={busy || report.archived}>{studentId ? 'ของนักเรียน' : s.displayName}</Button
					></Table.Cell
				><Table.Cell
					>{labels[s.category] ?? s.category}{s.scopeKey ? ` · ${s.scopeLabel}` : ''}</Table.Cell
				><Table.Cell>{s.present}</Table.Cell><Table.Cell>{s.late}</Table.Cell><Table.Cell
					>{s.absent}</Table.Cell
				><Table.Cell>{s.leave}</Table.Cell><Table.Cell>{s.activity}</Table.Cell><Table.Cell
					>{s.unchecked}</Table.Cell
				><Table.Cell>{s.expected}</Table.Cell><Table.Cell>{percent(s)}</Table.Cell></Table.Row
			>{:else}<Table.Row><Table.Cell colspan={10}>ยังไม่มีผลที่นับในสรุป</Table.Cell></Table.Row
			>{/each}</Table.Body
	></Table.Root
>
{#if selectedStudent && !report.archived}<section class="space-y-3 rounded-xl border p-4">
		<h2 class="font-semibold">รายละเอียดรายวัน</h2>
		<div class="flex flex-wrap items-end gap-3">
			<label>จาก<DatePicker bind:value={start} /></label><label
				>ถึง<DatePicker bind:value={end} /></label
			><Button onclick={() => loadHistory()} disabled={busy}>ดูรายละเอียด (ไม่เกิน 31 วัน)</Button>
		</div>
		{#each history as h (h.sessionId)}
			<div class="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-muted p-3">
				<span
					>{h.date} · {kindLabels[h.kind]} · {h.title} · {h.cancelled
						? 'งดเช็คชื่อ'
						: resultLabels[h.result]}
					{h.note}</span
				>{#if h.evidenceFileId}<Button variant="outline" onclick={() => preview(h)} disabled={busy}
						>ภาพตอนสแกน</Button
					>{/if}
			</div>{:else}<p>กดดูรายละเอียดเพื่อโหลดรายการ</p>{/each}{#if image}<div>
				<Button variant="outline" onclick={closeImage}>ปิดภาพ</Button><img
					src={image}
					alt="ภาพหลักฐานตอนสแกนเข้าโรงเรียน"
					class="mt-3 max-w-lg rounded-lg"
				/>
			</div>{/if}
	</section>{/if}
{#if canPurge}<section class="space-y-4 rounded-xl border p-5">
		<h2 class="font-semibold">ล้างรายละเอียดภาคเรียน</h2>
		<p class="text-sm text-muted-foreground">
			ทำได้หลังปิดภาคเรียน ระบบตรวจยอดและเก็บสรุปรายคนก่อนล้างผลรายครั้งและภาพหลักฐาน
		</p>
		<Button variant="outline" onclick={checkPurge} disabled={busy || report.archived}
			>ตรวจผลกระทบก่อนล้าง</Button
		>{#if impact}<p>
				ผล {impact.records} รายการ · ภาพ {impact.evidence} ภาพ ({(
					impact.evidenceBytes /
					1024 /
					1024
				).toFixed(1)} MB)
			</p>
			{#if impact.canPurge}<label class="block"
					>เหตุผล<Input bind:value={reason} maxlength={1000} /></label
				><label class="flex gap-2"
					><input
						type="checkbox"
						bind:checked={confirmed}
					/>ยืนยันล้างรายละเอียดและภาพของภาคเรียนนี้ โดยคงยอดสรุป</label
				><Button
					variant="destructive"
					onclick={purge}
					disabled={busy || !confirmed || !reason.trim()}>ล้างข้อมูลภาคเรียนนี้</Button
				>{:else}<p>ยังล้างไม่ได้ หรือภาคเรียนนี้ล้างแล้ว</p>{/if}{/if}
	</section>{/if}
