<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
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
	import AttendanceSelect from './AttendanceSelect.svelte';
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
	let appliedSearch = $state(''),
		category = $state('all'),
		appliedCategory = $state('all'),
		exported = $state(0);
	let imageGeneration = 0,
		disposed = false;
	const historyRequest = new LatestRequest();
	const reportRequest = new LatestRequest();
	const exportRequest = new LatestRequest();
	let lastSummaryQuery = $state({ page: untrack(() => initial.page), search: '', category: 'all' });
	const labels: Record<string, string> = {
		school: 'มาโรงเรียน (วัน)',
		flag: 'หน้าเสาธง (ครั้ง)',
		lesson: 'รายวิชา (คาบ)',
		special: 'รอบพิเศษ (ครั้ง)'
	};
	const summaries = $derived(report.summaries);
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
		if (disposed || busy) return;
		busy = true;
		pendingAction = name;
		errors[name.startsWith('image:') ? 'image' : name] = '';
		try {
			await action();
		} catch (e) {
			if (disposed) return;
			const message =
				name === 'export' && e instanceof Error && e.name === 'AbortError'
					? 'ยกเลิกการส่งออกแล้ว'
					: e instanceof Error
						? e.message
						: 'โหลดไม่สำเร็จ';
			errors[name.startsWith('image:') ? 'image' : name] = message;
		} finally {
			if (!disposed) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function loadHistory(id = selectedStudent) {
		if (disposed || !id || busy) return;
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
		if (disposed || busy || !s.evidenceFileId) return;
		const token = ++imageGeneration;
		await run(`image:${s.sessionId}`, async () => {
			const blob = await downloadFile(s.evidenceFileId!, selectedStudent);
			if (disposed || token !== imageGeneration) return;
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
		disposed = true;
		closeImage();
		historyRequest.abort();
		reportRequest.abort();
		exportRequest.abort();
	});
	async function refreshSummary(
		page = report.page,
		filter = { search: appliedSearch, category: appliedCategory }
	) {
		if (disposed) return;
		const ticket = reportRequest.begin();
		errors.summary = '';
		lastSummaryQuery = { page, ...filter };
		try {
			const latest = await attendanceReport(
				term,
				studentId,
				{ signal: ticket.signal },
				{
					page,
					pageSize: 50,
					search: filter.search,
					category: filter.category === 'all' ? undefined : filter.category
				}
			);
			if (reportRequest.isCurrent(ticket.revision)) {
				report = latest;
				appliedSearch = filter.search;
				appliedCategory = filter.category;
			}
		} catch (error) {
			if (reportRequest.isCurrent(ticket.revision))
				errors.summary = error instanceof Error ? error.message : 'โหลดสรุปล่าสุดไม่ได้';
		}
	}
	async function checkPurge() {
		if (disposed || !canPurge || report.archived || busy) return;
		impact = null;
		confirmed = false;
		errors.purge = '';
		await run('impact', async () => {
			const result = await attendancePurgeImpact(term);
			if (disposed) return;
			impact = result;
			confirmed = false;
		});
	}
	async function purge() {
		if (
			disposed ||
			!canPurge ||
			report.archived ||
			!impact?.canPurge ||
			!confirmed ||
			!reason.trim() ||
			busy
		)
			return;
		await run('purge', async () => {
			const result = await purgeAttendanceTerm(term, { expectedRecords: impact!.records, reason });
			if (disposed) return;
			impact = result;
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
	async function exportCsv() {
		await run('export', async () => {
			const ticket = exportRequest.begin();
			exported = 0;
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
			const quoteRow = (row: (string | number)[]) =>
				row
					.map((value) => {
						const text = String(value);
						const cell =
							typeof value === 'string' && /^[=+@\-\t\r]/u.test(text) ? "'" + text : text;
						return '"' + cell.replaceAll('"', '""') + '"';
					})
					.join(',');
			const chunks = ['\uFEFF' + quoteRow(header)];
			let page = 1,
				total: number | null = null;
			const seen = new SvelteSet<string>();
			do {
				const next = await attendanceReport(
					term,
					studentId,
					{ signal: ticket.signal },
					{
						page,
						pageSize: 100,
						search: appliedSearch,
						category: appliedCategory === 'all' ? undefined : appliedCategory
					}
				);
				if (!exportRequest.isCurrent(ticket.revision) || disposed) return;
				if (total !== null && total !== next.total)
					throw new Error('รายงานเปลี่ยนระหว่างส่งออก กรุณาลองส่งออกใหม่');
				total = next.total;
				if (!next.summaries.length && exported < total)
					throw new Error('รายงานเปลี่ยนระหว่างส่งออก กรุณาลองส่งออกใหม่');
				for (const row of next.summaries) {
					const key = JSON.stringify([row.studentId, row.category, row.scopeKey]);
					if (seen.has(key)) throw new Error('รายงานเปลี่ยนระหว่างส่งออก กรุณาลองส่งออกใหม่');
					seen.add(key);
				}
				const rows = next.summaries.map((s) => [
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
				for (const row of rows) chunks.push('\r\n' + quoteRow(row));
				exported += rows.length;
				page++;
			} while (exported < total);
			const url = URL.createObjectURL(new Blob(chunks, { type: 'text/csv;charset=utf-8' }));
			const a = document.createElement('a');
			a.href = url;
			a.download = 'attendance-summary.csv';
			a.click();
			URL.revokeObjectURL(url);
		});
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
		<Card.Content class="p-3 sm:p-4"
			><form
				class="flex flex-wrap items-end gap-3"
				onsubmit={(event) => {
					event.preventDefault();
					void run('summary', () => refreshSummary(1, { search: search.trim(), category }));
				}}
			>
				<div class="w-full min-w-0 space-y-2 sm:w-auto sm:flex-1">
					<Label for="attendance-report-search">ค้นหาสรุป</Label><Input
						id="attendance-report-search"
						bind:value={search}
						placeholder="ค้นหาชื่อ / รายวิชา / รอบ"
						maxlength={120}
						disabled={busy}
					/>
				</div>
				<div class="space-y-2">
					<Label for="attendance-report-category">ประเภท</Label><AttendanceSelect
						id="attendance-report-category"
						label="ประเภทสรุป"
						bind:value={category}
						disabled={busy}
						options={[
							{ value: 'all', label: 'ทุกประเภท' },
							...Object.entries(labels).map(([value, label]) => ({ value, label }))
						]}
					/>
				</div>
				<LoadingButton type="submit" loading={pendingAction === 'summary'} disabled={busy}
					>ค้นหา</LoadingButton
				>
				<LoadingButton
					type="button"
					variant="outline"
					loading={pendingAction === 'export'}
					disabled={busy}
					onclick={exportCsv}><Download class="size-4" />ส่งออก CSV</LoadingButton
				>
				{#if pendingAction === 'export'}<p role="status" class="text-sm text-muted-foreground">
						ส่งออกแล้ว {exported} รายการ
					</p>
					<Button type="button" variant="outline" onclick={() => exportRequest.abort()}
						>ยกเลิกการส่งออก</Button
					>{/if}
				{#if errors.export}<p role="alert" class="w-full text-sm text-destructive">
						{errors.export}
					</p>{/if}
			</form></Card.Content
		>
	</Card.Root>
	<Card.Root class="min-w-0">
		<Card.Header
			><Card.Title><h2>ยอดสรุปการเช็คชื่อ</h2></Card.Title><Card.Description
				>เลือกนักเรียนเพื่อดูรายละเอียดรายวัน · แสดงครั้งละ 50 รายการ</Card.Description
			></Card.Header
		>
		<Card.Content class="min-w-0 space-y-4">
			{#if errors.summary}<PageState
					variant="error"
					title="โหลดสรุปล่าสุดไม่ได้"
					description={errors.summary}
					actionLabel="ลองโหลดสรุปล่าสุดใหม่"
					onaction={() => {
						void run('summary', () => refreshSummary(lastSummaryQuery.page, lastSummaryQuery));
					}}
				/>{/if}
			<Table.Root class="block min-w-0 sm:table sm:min-w-[800px]"
				><Table.Header class="hidden sm:table-header-group"
					><Table.Row
						>{#each ['นักเรียน', 'ประเภท / รอบ', 'มา', 'สาย', 'ขาด', 'ลา', 'กิจกรรม', 'ยังไม่เช็ค', 'ทั้งหมด', 'ร้อยละ'] as label (label)}<Table.Head
								>{label}</Table.Head
							>{/each}</Table.Row
					></Table.Header
				>
				<Table.Body class="block sm:table-row-group"
					>{#each summaries as s (s.studentId + s.category + s.scopeKey)}<Table.Row
							class="grid grid-cols-4 gap-3 py-4 sm:table-row"
						>
							<Table.Cell class="col-span-4 block sm:table-cell"
								><Button
									type="button"
									variant="link"
									class="h-auto max-w-full justify-start whitespace-normal text-left"
									onclick={() => loadHistory(s.studentId)}
									disabled={busy || report.archived}
									>{studentId ? 'ของนักเรียน' : s.displayName}</Button
								></Table.Cell
							><Table.Cell class="col-span-4 block break-words sm:table-cell"
								>{labels[s.category] ?? s.category}{s.scopeKey
									? ` · ${s.scopeLabel}`
									: ''}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">มา</span
								>{s.present}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">สาย</span
								>{s.late}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">ขาด</span
								>{s.absent}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">ลา</span
								>{s.leave}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">กิจกรรม</span
								>{s.activity}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">ยังไม่เช็ค</span
								>{s.unchecked}</Table.Cell
							><Table.Cell class="block sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">ทั้งหมด</span
								>{s.expected}</Table.Cell
							><Table.Cell class="block font-medium sm:table-cell"
								><span class="block text-xs text-muted-foreground sm:hidden">ร้อยละ</span>{percent(
									s
								)}</Table.Cell
							>
						</Table.Row>{:else}<Table.Row
							><Table.Cell colspan={10} class="py-8 text-center text-muted-foreground"
								>{appliedSearch
									? 'ไม่พบข้อมูลที่ตรงกับคำค้น ลองเปลี่ยนคำค้น'
									: 'ยังไม่มีผลที่นับในสรุป'}</Table.Cell
							></Table.Row
						>{/each}</Table.Body
				>
			</Table.Root>
			<div class="flex flex-wrap items-center justify-between gap-3" aria-label="หน้ารายงาน">
				<p class="text-sm text-muted-foreground">หน้า {report.page} · {report.total} รายการ</p>
				<div class="flex gap-2">
					<Button
						type="button"
						variant="outline"
						disabled={busy || report.page <= 1}
						onclick={() => {
							void run('summary', () => refreshSummary(report.page - 1));
						}}>ก่อนหน้า</Button
					><Button
						type="button"
						variant="outline"
						disabled={busy || report.page * report.pageSize >= report.total}
						onclick={() => {
							void run('summary', () => refreshSummary(report.page + 1));
						}}>ถัดไป</Button
					>
				</div>
			</div>
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
