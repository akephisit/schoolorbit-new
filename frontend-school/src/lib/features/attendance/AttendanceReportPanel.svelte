<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { LoaderCircle, Download, Info } from '@lucide/svelte';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
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
		pendingAction = $state(''),
		errors = $state<Record<string, string>>({}),
		purgeOpen = $state(false),
		historyError = $state(''),
		historyLoaded = $state(false),
		start = $state(untrack(() => date.slice(0, 7)) + '-01'),
		end = $state(untrack(() => date)),
		impact = $state<AttendancePurgeImpact | null>(null),
		reason = $state(''),
		confirmed = $state(false),
		image = $state(''),
		search = $state('');
	let imageGeneration = 0;
	const historyRequest = new LatestRequest();
	const reportRequest = new LatestRequest();
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
	async function run(name: string, action: () => Promise<void>) {
		if (busy) return;
		busy = true;
		pendingAction = name;
		errors[name.startsWith('image:') ? 'image' : name] = '';
		try {
			await action();
		} catch (e) {
			const message = e instanceof Error ? e.message : 'โหลดไม่สำเร็จ';
			errors[name.startsWith('image:') ? 'image' : name] = message;
			toast.error(message);
		} finally {
			busy = false;
			pendingAction = '';
		}
	}
	async function loadHistory(id = selectedStudent) {
		if (!id || busy) return;
		const t = historyRequest.begin();
		closeImage();
		history = [];
		historyError = '';
		historyLoaded = false;
		selectedStudent = id;
		await run('history', async () => {
			try {
				const first = Date.parse(start + 'T00:00:00Z'),
					last = Date.parse(end + 'T00:00:00Z');
				if (
					!Number.isFinite(first) ||
					!Number.isFinite(last) ||
					last < first ||
					(last - first) / 86400000 >= 31
				)
					throw new Error('เลือกช่วงวันที่ไม่เกิน 31 วัน');
				const rows = await attendanceHistory(term, id, start, end, { signal: t.signal });
				if (historyRequest.isCurrent(t.revision)) {
					history = rows;
					historyLoaded = true;
				}
			} catch (e) {
				if (historyRequest.isCurrent(t.revision))
					historyError = e instanceof Error ? e.message : 'โหลดรายละเอียดไม่ได้';
			}
		});
	}
	async function preview(s: AttendanceHistoryItem) {
		if (!s.evidenceFileId) return;
		const token = ++imageGeneration;
		await run(`image:${s.sessionId}`, async () => {
			const blob = await downloadFile(s.evidenceFileId!, selectedStudent);
			if (token !== imageGeneration) return;
			if (image) URL.revokeObjectURL(image);
			image = URL.createObjectURL(blob);
		});
	}
	function closeImage() {
		imageGeneration++;
		errors.image = '';
		if (image) URL.revokeObjectURL(image);
		image = '';
	}
	onDestroy(() => {
		closeImage();
		historyRequest.abort();
		reportRequest.abort();
	});
	async function refreshSummary() {
		const ticket = reportRequest.begin();
		errors.summary = '';
		try {
			const latest = await attendanceReport(term, studentId, { signal: ticket.signal });
			if (reportRequest.isCurrent(ticket.revision)) report = latest;
		} catch (error) {
			if (reportRequest.isCurrent(ticket.revision))
				errors.summary = error instanceof Error ? error.message : 'โหลดสรุปล่าสุดไม่ได้';
		}
	}
	async function checkPurge() {
		if (!canPurge || report.archived || busy) return;
		impact = null;
		confirmed = false;
		errors.purge = '';
		await run('impact', async () => {
			impact = await attendancePurgeImpact(term);
			confirmed = false;
		});
	}
	async function purge() {
		if (!canPurge || report.archived || !impact?.canPurge || !confirmed || !reason.trim() || busy)
			return;
		await run('purge', async () => {
			impact = await purgeAttendanceTerm(term, { expectedRecords: impact!.records, reason });
			report = { ...report, archived: impact.archived };
			historyLoaded = false;
			purgeOpen = false;
			history = [];
			closeImage();
			toast.success('ล้างรายละเอียดแล้ว ยอดสรุปยังอยู่');
			confirmed = false;
			await refreshSummary();
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

<div class="min-w-0 space-y-6">
	<Card.Root class="gap-0 border-primary/20 bg-gradient-to-r from-primary/[0.08] to-card py-0">
		<Card.Content class="space-y-3 p-5 sm:p-6">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<p class="flex items-center gap-2 font-medium">
					<Info class="size-4 text-primary" />อ่านยอดสรุปการเช็คชื่อ
				</p>
				{#if report.archived}<Badge variant="secondary">เก็บยอดสรุปแล้ว</Badge>{/if}
			</div>
			<p class="text-sm leading-relaxed text-muted-foreground">
				ยอดมาโรงเรียน หน้าเสาธง รายคาบ และรอบพิเศษแยกกัน วันที่ไม่ประมวลผลและคาบที่งดไม่นับขาด
				“ยังไม่เช็ค” แสดงข้อมูลที่ครูยังไม่ได้บันทึก ร้อยละคำนวณเฉพาะผลที่ทราบแล้ว{report.archived
					? ' · ภาคเรียนนี้ล้างรายละเอียดแล้ว แต่ยังเก็บสรุปไว้'
					: ''}
			</p>
		</Card.Content>
	</Card.Root>
	<Card.Root class="gap-0 py-0">
		<Card.Content class="flex flex-wrap items-end gap-3 p-3 sm:p-4"
			><div class="min-w-0 flex-1 space-y-2">
				<Label for="attendance-report-search">ค้นหาสรุป</Label><Input
					id="attendance-report-search"
					bind:value={search}
					placeholder="ค้นหารายวิชา / รอบ"
				/>
			</div>
			<Button type="button" variant="outline" onclick={exportCsv}
				><Download class="size-4" />ส่งออก CSV</Button
			></Card.Content
		>
	</Card.Root>
	<Card.Root class="min-w-0">
		<Card.Header
			><Card.Title><h2>ยอดสรุปการเช็คชื่อ</h2></Card.Title><Card.Description
				>เลือกชื่อนักเรียนเพื่อดูรายละเอียดรายวัน เลื่อนตารางแนวนอนเพื่อดูข้อมูลครบทุกคอลัมน์</Card.Description
			></Card.Header
		>
		<Card.Content class="min-w-0 space-y-4">
			{#if errors.summary}<PageState
					variant="error"
					title="ล้างรายละเอียดแล้ว แต่โหลดสรุปล่าสุดไม่ได้"
					description={errors.summary}
					actionLabel="ลองโหลดสรุปล่าสุดใหม่"
					onaction={() => {
						void run('summary', refreshSummary);
					}}
				/>{/if}
			<Table.Root class="min-w-[800px]"
				><Table.Header
					><Table.Row
						>{#each ['นักเรียน', 'ประเภท / รอบ', 'มา', 'สาย', 'ขาด', 'ลา', 'กิจกรรม', 'ยังไม่เช็ค', 'ทั้งหมด', 'ร้อยละ'] as label (label)}<Table.Head
								>{label}</Table.Head
							>{/each}</Table.Row
					></Table.Header
				>
				<Table.Body
					>{#each summaries as s (s.studentId + s.category + s.scopeKey)}<Table.Row>
							<Table.Cell
								><Button
									type="button"
									variant="link"
									onclick={() => loadHistory(s.studentId)}
									disabled={busy || report.archived}
									>{studentId ? 'ของนักเรียน' : s.displayName}</Button
								></Table.Cell
							><Table.Cell
								>{labels[s.category] ?? s.category}{s.scopeKey
									? ` · ${s.scopeLabel}`
									: ''}</Table.Cell
							><Table.Cell>{s.present}</Table.Cell><Table.Cell>{s.late}</Table.Cell><Table.Cell
								>{s.absent}</Table.Cell
							><Table.Cell>{s.leave}</Table.Cell><Table.Cell>{s.activity}</Table.Cell><Table.Cell
								>{s.unchecked}</Table.Cell
							><Table.Cell>{s.expected}</Table.Cell><Table.Cell class="font-medium"
								>{percent(s)}</Table.Cell
							>
						</Table.Row>{:else}<Table.Row
							><Table.Cell colspan={10} class="py-8 text-center text-muted-foreground"
								>{search
									? 'ไม่พบข้อมูลที่ตรงกับคำค้น ลองเปลี่ยนคำค้น'
									: 'ยังไม่มีผลที่นับในสรุป'}</Table.Cell
							></Table.Row
						>{/each}</Table.Body
				>
			</Table.Root>
		</Card.Content>
	</Card.Root>
	{#if selectedStudent && !report.archived}
		<Card.Root>
			<Card.Header
				><Card.Title><h2>รายละเอียดรายวัน</h2></Card.Title><Card.Description
					>ดูผลรายครั้งและภาพหลักฐาน เฉพาะนักเรียนและช่วงวันที่ที่มีสิทธิ์เข้าถึง</Card.Description
				></Card.Header
			>
			<Card.Content class="space-y-6">
				<form
					onsubmit={(event) => {
						event.preventDefault();
						void loadHistory();
					}}
					class="flex flex-wrap items-end gap-3"
				>
					<div class="space-y-2">
						<Label for="attendance-history-start">จาก</Label><DatePicker
							id="attendance-history-start"
							ariaLabel="จาก"
							bind:value={start}
							disabled={busy}
						/>
					</div>
					<div class="space-y-2">
						<Label for="attendance-history-end">ถึง</Label><DatePicker
							id="attendance-history-end"
							ariaLabel="ถึง"
							bind:value={end}
							disabled={busy}
						/>
					</div>
					<LoadingButton type="submit" loading={pendingAction === 'history'} disabled={busy}
						>ดูรายละเอียด (ไม่เกิน 31 วัน)</LoadingButton
					>
				</form>
				{#if historyError}<PageState
						variant="error"
						title="โหลดรายละเอียดไม่ได้"
						description={historyError}
						actionLabel="ลองโหลดรายละเอียดใหม่"
						onaction={() => {
							void loadHistory();
						}}
					/>{/if}
				{#if pendingAction === 'history'}<p role="status" class="text-sm text-muted-foreground">
						กำลังโหลดรายละเอียด…
					</p>{/if}
				<div class="space-y-3">
					{#each history as h (h.sessionId)}<div
							class="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-muted/30 p-4"
						>
							<div class="min-w-0 space-y-2">
								<p class="break-words font-medium">{h.date} · {kindLabels[h.kind]} · {h.title}</p>
								<div class="flex flex-wrap items-center gap-2">
									<Badge
										variant={h.cancelled || h.result === 'unchecked'
											? 'outline'
											: h.result === 'absent'
												? 'destructive'
												: 'secondary'}>{h.cancelled ? 'งดเช็คชื่อ' : resultLabels[h.result]}</Badge
									>{#if h.note}<p class="break-words text-sm text-muted-foreground">
											{h.note}
										</p>{/if}
								</div>
							</div>
							{#if h.evidenceFileId}<LoadingButton
									type="button"
									variant="outline"
									onclick={() => preview(h)}
									loading={pendingAction === `image:${h.sessionId}`}
									disabled={busy}>ภาพตอนสแกน</LoadingButton
								>{/if}
						</div>{:else}{#if !historyError && pendingAction !== 'history'}<p
								class="text-sm text-muted-foreground"
							>
								{historyLoaded ? 'ไม่มีรายการในช่วงวันที่เลือก' : 'กดดูรายละเอียดเพื่อโหลดรายการ'}
							</p>{/if}{/each}
				</div>
				{#if errors.image}<Alert.Root variant="destructive"
						><Alert.Title>โหลดภาพหลักฐานไม่ได้</Alert.Title><Alert.Description
							>{errors.image}</Alert.Description
						></Alert.Root
					>{/if}
				{#if image}<div class="space-y-3 border-t pt-4">
						<Button type="button" variant="outline" onclick={closeImage}>ปิดภาพ</Button><img
							src={image}
							alt="ภาพหลักฐานตอนสแกนเข้าโรงเรียน"
							class="w-full max-w-lg rounded-lg"
						/>
					</div>{/if}
			</Card.Content>
		</Card.Root>
	{/if}
	{#if canPurge}
		<Card.Root>
			<Card.Header
				><Card.Title><h2>ล้างรายละเอียดภาคเรียน</h2></Card.Title><Card.Description
					>ทำได้หลังปิดภาคเรียน ระบบตรวจยอดและเก็บสรุปรายคนก่อนล้างผลรายครั้งและภาพหลักฐาน</Card.Description
				></Card.Header
			>
			<Card.Content class="space-y-4">
				{#if errors.impact}<Alert.Root variant="destructive"
						><Alert.Title>ตรวจผลกระทบไม่ได้</Alert.Title><Alert.Description
							>{errors.impact}</Alert.Description
						></Alert.Root
					>{/if}
				{#if errors.purge && !purgeOpen}<Alert.Root variant="destructive"
						><Alert.Title>ล้างรายละเอียดไม่ได้</Alert.Title><Alert.Description
							>{errors.purge}</Alert.Description
						></Alert.Root
					>{/if}
				<LoadingButton
					type="button"
					variant="outline"
					onclick={checkPurge}
					loading={pendingAction === 'impact'}
					disabled={busy || report.archived}>ตรวจผลกระทบก่อนล้าง</LoadingButton
				>
				{#if impact}
					<p class="text-sm">
						ผล {impact.records} รายการ · ภาพ {impact.evidence} ภาพ ({(
							impact.evidenceBytes /
							1024 /
							1024
						).toFixed(1)} MB)
					</p>
					{#if impact.canPurge}
						<form
							onsubmit={(event) => {
								event.preventDefault();
								if (!busy && confirmed && reason.trim()) {
									errors.purge = '';
									purgeOpen = true;
								}
							}}
							class="space-y-4"
						>
							<div class="space-y-2">
								<Label for="attendance-purge-reason"
									>เหตุผล <span class="text-destructive">*</span></Label
								><Input
									id="attendance-purge-reason"
									bind:value={reason}
									maxlength={1000}
									required
									disabled={busy}
								/>
							</div>
							<div class="flex items-start gap-2">
								<Checkbox
									id="attendance-purge-confirmed"
									bind:checked={confirmed}
									disabled={busy}
								/><Label for="attendance-purge-confirmed" class="leading-relaxed"
									>ยืนยันล้างรายละเอียดและภาพของภาคเรียนนี้ โดยคงยอดสรุป</Label
								>
							</div>
							<Button
								type="submit"
								variant="destructive"
								disabled={busy || !confirmed || !reason.trim()}>ล้างข้อมูลภาคเรียนนี้</Button
							>
						</form>
					{:else}<p class="text-sm text-muted-foreground">
							ยังล้างไม่ได้ หรือภาคเรียนนี้ล้างแล้ว
						</p>{/if}
				{/if}
			</Card.Content>
		</Card.Root>
	{/if}
</div>

<AlertDialog.Root
	open={purgeOpen && canPurge}
	onOpenChange={(open) => {
		if (!busy) purgeOpen = open;
	}}
>
	<AlertDialog.Content>
		<AlertDialog.Header
			><AlertDialog.Title>ยืนยันล้างรายละเอียดภาคเรียน?</AlertDialog.Title><AlertDialog.Description
				>ผลรายครั้ง {impact?.records ?? 0} รายการ และภาพหลักฐาน {impact?.evidence ?? 0} ภาพจะถูกล้าง การดำเนินการนี้ย้อนกลับไม่ได้
				ระบบยังคงยอดสรุปรายคนไว้</AlertDialog.Description
			></AlertDialog.Header
		>
		{#if errors.purge}<Alert.Root variant="destructive"
				><Alert.Title>ล้างรายละเอียดไม่ได้</Alert.Title><Alert.Description
					>{errors.purge}</Alert.Description
				></Alert.Root
			>{/if}
		<AlertDialog.Footer
			><AlertDialog.Cancel disabled={busy}>ยกเลิก</AlertDialog.Cancel><AlertDialog.Action
				variant="destructive"
				disabled={busy || !impact?.canPurge || !confirmed || !reason.trim()}
				onclick={(event) => {
					event.preventDefault();
					void purge();
				}}
				>{#if pendingAction === 'purge'}<LoaderCircle
						class="size-4 animate-spin"
					/>กำลังล้างรายละเอียด…{:else}ยืนยันล้างข้อมูล{/if}</AlertDialog.Action
			></AlertDialog.Footer
		>
	</AlertDialog.Content>
</AlertDialog.Root>
