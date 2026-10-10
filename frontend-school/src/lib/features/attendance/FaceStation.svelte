<script lang="ts">
	import { Camera, ShieldCheck, ScanFace } from '@lucide/svelte';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Alert, AlertDescription, AlertTitle } from '#lib/components/ui/alert/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import AttendanceSelect from './AttendanceSelect.svelte';
	import { onDestroy, untrack } from 'svelte';
	import { get } from 'svelte/store';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { uploadFile, deleteFile } from '#lib/api/files.js';
	import {
		enrollAttendanceFace,
		removeAttendanceFace,
		openAttendanceKiosk,
		currentAttendanceDate,
		scanAttendance,
		resultLabels,
		type AttendanceOptions,
		type AttendanceKioskWorkspace,
		type AttendanceScan
	} from '#lib/api/attendance.js';
	import {
		FACE_MODEL,
		camera,
		stopCamera,
		readFace,
		faceEngine,
		matchFace,
		evidence,
		distance
	} from './face-camera.js';
	let { term, date, initial }: { term: string; date: string; initial: AttendanceOptions } =
		$props();
	let video: HTMLVideoElement,
		stream: MediaStream | null = null,
		device = $state(''),
		student = $state(''),
		consent = $state(false),
		confirmRemoval = $state(false),
		active = $state(false),
		busy = $state(false),
		pendingAction = $state(''),
		running = $state(false),
		message = $state('เปิดเว็บแคมเพื่อเริ่มต้น'),
		gallery = $state<AttendanceKioskWorkspace | null>(null),
		scanDate = $state(untrack(() => date)),
		samples = $state<number[][]>([]),
		mode = $state<'scan' | 'enroll'>(
			untrack(() => (get(can).has(PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED) ? 'scan' : 'enroll'))
		),
		sampleTarget = $state<'center' | 'turn' | 'return'>('center'),
		challenge = $state(''),
		lastStudent = $state(''),
		committing = $state(false);
	let timer: ReturnType<typeof setTimeout> | null = null,
		generation = 0;
	let cameraController: AbortController | null = null;
	let pending = $state<{ payload: AttendanceScan; name: string } | null>(null);
	const seen = new SvelteSet<string>();
	const enrollAllowed = $derived(
		$can.hasAny(PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED, PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL)
	);
	const scanAllowed = $derived($can.has(PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED));
	onDestroy(() => {
		generation++;
		cameraController?.abort();
		running = false;
		if (timer) clearTimeout(timer);
		stopCamera(stream, video);
		pending = null;
		gallery = null;
		samples = [];
	});
	async function start() {
		const token = ++generation;
		cameraController?.abort();
		const controller = new AbortController();
		cameraController = controller;
		busy = true;
		pendingAction = 'camera';
		try {
			await faceEngine();
			if (token !== generation) return;
			const opened = await camera(video, controller.signal);
			if (token !== generation) {
				stopCamera(opened, video);
				return;
			}
			stream = opened;
			active = true;
			message = 'กล้องพร้อมแล้ว';
		} catch (e) {
			if (token !== generation) return;
			message = e instanceof Error ? e.message : 'เปิดกล้องไม่ได้';
		} finally {
			if (token === generation) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	function stop() {
		generation++;
		busy = false;
		pendingAction = '';
		cameraController?.abort();
		running = false;
		if (timer) clearTimeout(timer);
		stopCamera(stream, video);
		stream = null;
		active = false;
		gallery = null;
		samples = [];
		pending = null;
		message = 'ปิดกล้องแล้ว';
	}
	async function collect() {
		if (!student || !consent) {
			message = 'เลือกนักเรียนและยืนยันการลงทะเบียน';
			return;
		}
		if (busy || !active) return;
		const token = generation,
			selected = student;
		busy = true;
		pendingAction = 'enroll';
		try {
			const face = await readFace(video);
			if (token !== generation || selected !== student || !active) return;
			if (!face) throw new Error('ต้องเห็นใบหน้าชัดเจนเพียงคนเดียว');
			if (samples.length && Math.abs(face.yaw) < 0.04)
				throw new Error('หันหน้าซ้ายหรือขวาเล็กน้อยเพื่อเก็บมุมเพิ่ม');
			if (samples.length && distance(samples[0], face.values) > 0.55)
				throw new Error('ใบหน้าเปลี่ยนไป กรุณาเริ่มเก็บใหม่');
			samples = [...samples, face.values];
			message = `เก็บ ${samples.length}/3 ตัวอย่างแล้ว`;
			if (samples.length >= 3) {
				await enrollAttendanceFace(selected, {
					model: FACE_MODEL,
					descriptors: samples.map((values) => ({ values })),
					consentConfirmed: consent
				});
				if (token !== generation) return;
				message = 'ลงทะเบียนใบหน้าแล้ว';
				toast.success(message);
				samples = [];
				consent = false;
			}
		} catch (e) {
			if (token === generation) message = e instanceof Error ? e.message : 'เก็บใบหน้าไม่ได้';
		} finally {
			if (token === generation) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function remove() {
		if (!student || !consent || busy || !enrollAllowed) return;
		const token = generation,
			selected = student;
		busy = true;
		pendingAction = 'withdraw';
		try {
			await removeAttendanceFace(selected);
			if (token !== generation || selected !== student) return;
			message = 'ลบข้อมูลใบหน้าแล้ว';
			toast.success(message);
			samples = [];
			consent = false;
		} catch (e) {
			if (token === generation) message = e instanceof Error ? e.message : 'ถอนการลงทะเบียนไม่ได้';
		} finally {
			if (token === generation) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function beginScanning() {
		if (!scanAllowed || !active || busy || !device) return;
		scanDate = currentAttendanceDate();
		const token = ++generation;
		busy = true;
		pendingAction = 'kiosk';
		try {
			const opened = await openAttendanceKiosk({
				academicTermId: term,
				date: scanDate,
				deviceId: device
			});
			if (token !== generation || !active) return;
			gallery = opened;
			if (!gallery.faces.length) throw new Error('ยังไม่มีใบหน้าที่ลงทะเบียน');
			seen.clear();
			running = true;
			challenge = Math.random() < 0.5 ? 'ซ้าย' : 'ขวา';
			sampleTarget = 'center';
			lastStudent = '';
			message = 'มองตรงเข้ากล้องทีละคน';
			await tick(generation);
		} catch (e) {
			if (token !== generation) return;
			message = e instanceof Error ? e.message : 'เปิดเครื่องสแกนไม่ได้';
			running = false;
		} finally {
			if (token === generation) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	function currentScanDay() {
		if (scanDate === currentAttendanceDate()) return true;
		stop();
		message = 'เปลี่ยนวันแล้ว กรุณาเปิดกล้องและเริ่มเช็คชื่อวันนี้ใหม่';
		return false;
	}
	async function commit() {
		if (!pending || !scanAllowed || !currentScanDay()) return;
		const queued = pending;
		committing = true;
		let result;
		try {
			result = await scanAttendance(queued.payload);
		} finally {
			committing = false;
		}
		seen.add(result.studentId);
		if (pending !== queued) return;
		message = result.teacherConflict
			? `${queued.name}: บันทึกหลักฐานแล้ว แต่คงผลของครู (${resultLabels[result.result]})`
			: result.duplicate
				? `${queued.name}: เช็คชื่อแล้ว`
				: `${queued.name}: บันทึกสำเร็จ · ${resultLabels[result.result]}`;
		pending = null;
		sampleTarget = 'center';
		lastStudent = '';
	}
	async function retry() {
		if (!currentScanDay()) return;
		busy = true;
		try {
			await commit();
			if (active && running) {
				generation++;
				await tick(generation);
			}
		} catch (e) {
			message = e instanceof Error ? e.message : 'ยังบันทึกไม่ได้ กดลองส่งอีกครั้ง';
		} finally {
			busy = false;
		}
	}
	async function tick(token: number) {
		if (token !== generation || !running || !gallery || !scanAllowed || !currentScanDay()) return;
		try {
			if (pending) return;
			const face = await readFace(video);
			if (token !== generation || !currentScanDay()) return;
			if (!face) {
				sampleTarget = 'center';
				lastStudent = '';
			} else {
				const id = matchFace(gallery.faces, face.values, gallery.configuration);
				if (id && !seen.has(id)) {
					if (sampleTarget === 'center' || id !== lastStudent) {
						if (Math.abs(face.yaw) < 0.08) {
							lastStudent = id;
							sampleTarget = 'turn';
							message = `หันหน้า${challenge}เล็กน้อย แล้วกลับมองตรง`;
						}
					} else if (
						sampleTarget === 'turn' &&
						((challenge === 'ซ้าย' && face.yaw < -0.06) || (challenge === 'ขวา' && face.yaw > 0.06))
					) {
						sampleTarget = 'return';
						message = 'กลับมองตรงเพื่อบันทึก';
					} else if (sampleTarget === 'return' && Math.abs(face.yaw) < 0.06) {
						const s = gallery.students.find((s) => s.id === id);
						const session = gallery.sessions.find((r) => r.homeroomId === s?.homeroomId);
						if (!s || !session) throw new Error('นักเรียนยังไม่มีห้องประจำชั้นในวันนี้');
						const capturedAt = new Date().toISOString();
						const image = await evidence(video);
						if (token !== generation || !currentScanDay()) return;
						const file = await uploadFile(image, 'attendance_evidence', id);
						if (token !== generation || !currentScanDay()) {
							await deleteFile(file.id, id);
							return;
						}
						pending = {
							name: s.name,
							payload: {
								eventId: crypto.randomUUID(),
								deviceId: device,
								sessionId: session.id,
								studentId: id,
								descriptor: { values: face.values },
								evidenceFileId: file.id,
								capturedAt
							}
						};
						message = 'กำลังบันทึก…';
						await commit();
						challenge = Math.random() < 0.5 ? 'ซ้าย' : 'ขวา';
					}
				} else if (id && seen.has(id)) {
					message = 'เช็คชื่อแล้ว เชิญคนถัดไป';
					sampleTarget = 'center';
					lastStudent = '';
				} else {
					message = 'จับคู่ไม่ได้ กรุณามองกล้องใหม่ หรือให้ครูเช็คชื่อ';
					sampleTarget = 'center';
					lastStudent = '';
				}
			}
		} catch (e) {
			message = e instanceof Error ? e.message : 'สแกนไม่ได้';
			if (pending) {
				message += ' · กดส่งรายการเดิมอีกครั้ง';
				return;
			}
		}
		if (token === generation && running)
			timer = setTimeout(() => {
				void tick(token);
			}, 700);
	}
</script>

<Alert class="border-primary/20 bg-gradient-to-r from-primary/10 to-card">
	<ShieldCheck class="text-primary" /><AlertTitle>เช็คชื่อโดยมีครูดูแล</AlertTitle><AlertDescription
		>ใช้คอมพิวเตอร์กับเว็บแคมให้เห็นทีละคน และมีครูดูแล กรณีจับคู่ไม่ได้ให้ครูเช็คชื่อด้วยมือ
		การหันหน้าช่วยตรวจภาพนิ่ง แต่ยังต้องทดสอบแสงและความแม่นยำก่อนใช้เป็นหลัก</AlertDescription
	>
</Alert>
<div class="flex flex-wrap gap-2" aria-label="วิธีใช้งานเว็บแคม">
	{#if scanAllowed}<Button
			type="button"
			variant={mode === 'scan' ? 'secondary' : 'outline'}
			aria-pressed={mode === 'scan'}
			disabled={running || busy}
			onclick={() => {
				mode = 'scan';
				samples = [];
			}}><ScanFace class="size-4" />สแกนเข้าโรงเรียน</Button
		>{/if}
	{#if enrollAllowed}<Button
			type="button"
			variant={mode === 'enroll' ? 'secondary' : 'outline'}
			aria-pressed={mode === 'enroll'}
			disabled={running || busy}
			onclick={() => {
				mode = 'enroll';
			}}>ลงทะเบียนใบหน้า</Button
		>{/if}
</div>
<div class="grid items-start gap-6 xl:grid-cols-2">
	<Card class="min-w-0">
		<CardHeader
			><CardTitle
				><h2 class="flex items-center gap-2">
					<Camera class="size-4 text-primary" />ภาพสดจากเว็บแคม
				</h2></CardTitle
			><CardDescription>วางใบหน้าให้อยู่ในกรอบและให้แสงสว่างเพียงพอ</CardDescription></CardHeader
		>
		<CardContent class="space-y-4">
			<video
				bind:this={video}
				class="aspect-[4/3] w-full rounded-lg bg-black"
				autoplay
				muted
				playsinline
				aria-label="ภาพสดจากเว็บแคม"><track kind="captions" /></video
			>
			<p
				role="status"
				aria-live="polite"
				class="rounded-lg border bg-muted/30 p-4 text-sm font-medium"
			>
				{message}
			</p>
			<div class="flex flex-wrap gap-2">
				<LoadingButton
					type="button"
					loading={pendingAction === 'camera'}
					onclick={start}
					disabled={busy || active}>เปิดเว็บแคม</LoadingButton
				><Button type="button" variant="outline" onclick={stop} disabled={!active && !busy}
					>หยุดและปิดกล้อง</Button
				>{#if pending}<LoadingButton
						type="button"
						loading={committing}
						onclick={retry}
						disabled={busy || committing}>ส่งรายการเดิมอีกครั้ง</LoadingButton
					>{/if}
			</div>
		</CardContent>
	</Card>
	<Card class="min-w-0">
		<CardHeader
			><CardTitle
				><h2>
					{mode === 'scan' ? 'เริ่มเช็คชื่อเข้าโรงเรียน' : 'ลงทะเบียนและจัดการใบหน้า'}
				</h2></CardTitle
			><CardDescription
				>{mode === 'scan'
					? 'เลือกเครื่องที่ได้รับมอบหมายแล้วเริ่มเช็คชื่อทีละคน'
					: 'เลือกนักเรียนและยืนยันความยินยอมก่อนเก็บตัวอย่าง'}</CardDescription
			></CardHeader
		>
		<CardContent class="space-y-5">
			{#if mode === 'scan'}
				<div class="space-y-2">
					<Label for="attendance-camera-device">เครื่องสแกน</Label><AttendanceSelect
						id="attendance-camera-device"
						label="เครื่องสแกน"
						placeholder="เลือกเครื่องที่ได้รับมอบหมาย"
						bind:value={device}
						disabled={running || busy}
						options={initial.devices
							.filter((d) => d.enabled)
							.map((d) => ({ value: d.id, label: d.name }))}
					/>
				</div>
				<LoadingButton
					type="button"
					loading={pendingAction === 'kiosk'}
					onclick={beginScanning}
					disabled={!active || !device || busy || running || !scanAllowed}
					>เริ่มเช็คชื่อวันนี้</LoadingButton
				>
				<div class="space-y-2 rounded-lg bg-muted/30 p-4 text-sm">
					<p class="font-medium">วันที่เช็คชื่อ: {formatCalendarDate(scanDate)}</p>
					<p class="text-muted-foreground">
						ทำงานทีละคน เก็บภาพเฉพาะรายการที่ส่งบันทึก
						แจ้งนักเรียนและผู้ปกครองทุกคนที่เชื่อมกับนักเรียน เมื่อหยุดกล้อง
						ข้อมูลใบหน้าในหน้านี้จะถูกล้าง
					</p>
				</div>
			{:else}
				<div class="space-y-2">
					<Label for="attendance-camera-student">นักเรียน</Label><AttendanceSelect
						id="attendance-camera-student"
						label="นักเรียน"
						placeholder="เลือกนักเรียน"
						disabled={busy}
						bind:value={student}
						onValueChange={() => {
							samples = [];
							consent = false;
						}}
						options={initial.students.map((s) => ({
							value: s.id,
							label: `${s.homeroomName ?? '-'} · ${s.name}`
						}))}
					/>
				</div>
				<div class="flex items-start gap-3 rounded-lg border p-4">
					<Checkbox id="attendance-camera-consent" bind:checked={consent} disabled={busy} /><Label
						for="attendance-camera-consent"
						class="leading-relaxed">ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก</Label
					>
				</div>
				<p class="text-sm text-muted-foreground">
					เก็บ 3 ตัวอย่าง: หน้าตรง แล้วหันซ้ายและขวาเล็กน้อย
				</p>
				<div class="flex flex-wrap gap-2">
					<LoadingButton
						type="button"
						loading={pendingAction === 'enroll'}
						onclick={collect}
						disabled={!active || !student || !consent || busy || !enrollAllowed}
						>เก็บตัวอย่าง ({samples.length}/3)</LoadingButton
					><Button
						type="button"
						variant="outline"
						onclick={() => {
							samples = [];
						}}
						disabled={busy}>เริ่มเก็บใหม่</Button
					>
				</div>
				<div class="space-y-3 border-t pt-4">
					<p class="text-sm text-muted-foreground">
						ถอนข้อมูลใบหน้าของนักเรียนที่เลือกได้เมื่อยืนยันความยินยอมแล้ว
						ระบบจะให้ตรวจสอบอีกครั้งก่อนลบ
					</p>
					<Button
						type="button"
						variant="destructive"
						onclick={() => {
							confirmRemoval = true;
						}}
						disabled={!student || !consent || busy || !enrollAllowed}>ถอนการลงทะเบียนใบหน้า</Button
					>
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
<AlertDialog.Root bind:open={confirmRemoval}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยันถอนการลงทะเบียนใบหน้า</AlertDialog.Title><AlertDialog.Description
				>ลบข้อมูลใบหน้าของ {initial.students.find((s) => s.id === student)?.name} นักเรียนจะต้องลงทะเบียนและให้ความยินยอมใหม่ก่อนใช้เว็บแคมเช็คชื่อ
				การเช็คชื่อด้วยมือยังใช้งานได้</AlertDialog.Description
			></AlertDialog.Header
		><AlertDialog.Footer
			><AlertDialog.Cancel>ยกเลิก</AlertDialog.Cancel><AlertDialog.Action
				variant="destructive"
				onclick={remove}
				disabled={!student || !consent || busy || !enrollAllowed}
				>ยืนยันถอนข้อมูลใบหน้า</AlertDialog.Action
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
