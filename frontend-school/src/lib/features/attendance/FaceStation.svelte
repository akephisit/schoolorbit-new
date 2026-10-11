<script lang="ts">
	import { Camera, ShieldCheck, ScanFace, ArrowLeft, ArrowRight, Focus } from '@lucide/svelte';
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
	import FaceEnrollmentList from './FaceEnrollmentList.svelte';
	import AttendanceRouteRegion from './AttendanceRouteRegion.svelte';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { readAttendance, type AttendanceRead } from './attendance-access.js';
	import { FaceTracker, enrollmentPose } from './face-tracking.js';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { get } from 'svelte/store';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { ApiClientError } from '#lib/api/client.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { uploadFile, deleteFile } from '#lib/api/files.js';
	import {
		attendanceFaceEnrollments,
		type AttendanceFaceEnrollment,
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
		readFaces,
		type CameraFrame,
		faceEngine,
		matchFace,
		evidence,
		distance
	} from './face-camera.js';
	let {
		term,
		date,
		initial,
		initialEnrollments
	}: {
		term: string;
		date: string;
		initial: AttendanceOptions;
		initialEnrollments: Promise<RouteLoadResult<AttendanceRead<AttendanceFaceEnrollment[]>>>;
	} = $props();
	let enrollmentSource = $state.raw(untrack(() => initialEnrollments));
	const enrollmentRequest = new LatestRequest();
	let previousEnrollments = untrack(() => initialEnrollments);
	$effect.pre(() => {
		if (initialEnrollments !== previousEnrollments) {
			enrollmentRequest.abort();
			previousEnrollments = initialEnrollments;
			enrollmentSource = initialEnrollments;
		}
	});
	let removalStudent = $state('');
	let scanMethod = $state('automatic');
	let cameraAspectRatio = $state('4 / 3');
	let outlines = $state<{ box: import('./face-camera.js').FaceBox; label: string }[]>([]);
	const tracker = new FaceTracker();

	function reloadEnrollments() {
		const request = enrollmentRequest.begin();
		enrollmentSource = captureRouteLoad(
			readAttendance(
				[PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED, PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL],
				() => attendanceFaceEnrollments(term, { signal: request.signal })
			),
			'โหลดรายชื่อที่ลงทะเบียนใบหน้าไม่ได้'
		);
	}
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
	const poseHint = $derived(
		samples.length === 0
			? 'มองตรงเข้ากล้อง'
			: samples.length === 1
				? 'หันซ้ายของคุณเล็กน้อย'
				: 'หันขวาของคุณเล็กน้อย'
	);
	let timer: ReturnType<typeof setTimeout> | null = null,
		generation = 0,
		commitGeneration = 0;
	let cameraController: AbortController | null = null;
	let pending = $state<{ payload: AttendanceScan; name: string } | null>(null);
	const seen = new SvelteSet<string>();
	const enrollAllowed = $derived(
		$can.hasAny(PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED, PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL)
	);
	const scanAllowed = $derived($can.has(PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED));
	onDestroy(() => {
		enrollmentRequest.abort();
		generation++;
		cameraController?.abort();
		running = false;
		if (timer) clearTimeout(timer);
		stopCamera(stream, video);
		pending = null;
		gallery = null;
		samples = [];
		outlines = [];
		tracker.clear();
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
		commitGeneration++;
		committing = false;
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
		outlines = [];
		tracker.clear();
	}
	async function collect() {
		if (!enrollAllowed || !student || !consent) {
			message = 'เลือกนักเรียนและยืนยันการลงทะเบียน';
			return;
		}
		if (busy || !active) return;
		const token = generation,
			selected = student;
		busy = true;
		pendingAction = 'enroll';
		try {
			if (samples.length < 3) {
				const face = await readFace(video, cameraController?.signal);
				if (token !== generation || selected !== student || !active) return;
				if (!face) throw new Error('ต้องเห็นใบหน้าชัดเจนเพียงคนเดียว');
				if (!enrollmentPose(face.yaw, samples.length)) throw new Error(poseHint);
				if (samples.length && distance(samples[0], face.values) > 0.55)
					throw new Error('ใบหน้าเปลี่ยนไป กรุณาเริ่มเก็บใหม่');
				samples = [...samples, face.values];
				message = `เก็บ ${samples.length}/3 ตัวอย่างแล้ว`;
			}
			if (samples.length === 3) {
				await enrollAttendanceFace(selected, {
					model: FACE_MODEL,
					descriptors: samples.map((values) => ({ values })),
					consentConfirmed: consent
				});
				if (token !== generation) return;
				message = 'ลงทะเบียนใบหน้าแล้ว';
				toast.success(message);
				reloadEnrollments();
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
		if (!removalStudent || busy || !enrollAllowed) return;
		// Withdrawal must dispose cached descriptors and any in-flight camera generation first.
		if (running || gallery || pending) stop();
		const token = generation,
			selected = removalStudent;
		busy = true;
		pendingAction = 'withdraw';
		try {
			await removeAttendanceFace(selected);
			if (token !== generation) return;
			reloadEnrollments();
			message = 'ลบข้อมูลใบหน้าแล้ว';
			toast.success(message);
			if (selected === student) {
				samples = [];
				consent = false;
			}
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
			tracker.clear();
			running = true;
			challenge = Math.random() < 0.5 ? 'ซ้าย' : 'ขวา';
			sampleTarget = 'center';
			lastStudent = '';
			message =
				scanMethod === 'automatic'
					? 'มองเข้ากล้อง ระบบยืนยันใบหน้าต่อเนื่องแล้วบันทึกอัตโนมัติ'
					: 'มองตรงเข้ากล้องทีละคน';
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
		const commitToken = ++commitGeneration;
		committing = true;
		let result;
		try {
			result = await scanAttendance(queued.payload);
		} catch (error) {
			if (
				pending === queued &&
				error instanceof ApiClientError &&
				[400, 403, 404, 409, 422].includes(error.status)
			) {
				pending = null;
				running = false;
				gallery = null;
				sampleTarget = 'center';
				lastStudent = '';
				message = error.message + ' · กดเริ่มเช็คชื่อวันนี้เพื่อถ่ายภาพใหม่';
				return;
			}
			throw error;
		} finally {
			if (commitToken === commitGeneration) committing = false;
		}
		if (pending !== queued) return;
		seen.add(result.studentId);
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
		const token = generation;
		busy = true;
		try {
			await commit();
			if (token === generation && active && running) {
				await tick(token);
			}
		} catch (e) {
			if (token !== generation) return;
			message = e instanceof Error ? e.message : 'ยังบันทึกไม่ได้ กดลองส่งอีกครั้ง';
		} finally {
			if (token === generation) busy = false;
		}
	}
	async function acceptFace(
		capture: CameraFrame,
		face: CameraFrame['faces'][number],
		id: string,
		token: number
	) {
		if (!gallery || pending || seen.has(id)) return;
		const s = gallery.students.find((s) => s.id === id);
		const session = gallery.sessions.find((r) => r.homeroomId === s?.homeroomId);
		if (!s || !session) throw new Error('นักเรียนยังไม่มีห้องประจำชั้นในวันนี้');
		const image = await evidence(capture.frame, face.box);
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
				rosterToken: session.rosterToken,
				session: {
					academicTermId: session.academicTermId,
					date: session.date,
					kind: session.kind,
					sourceKey: session.sourceKey
				},
				studentId: id,
				descriptor: { values: face.values },
				evidenceFileId: file.id,
				capturedAt: capture.capturedAt
			}
		};
		message = 'กำลังบันทึก…';
		await commit();
	}
	async function tick(token: number) {
		if (token !== generation || !running || !gallery || !scanAllowed || !currentScanDay()) return;
		const started = performance.now();
		try {
			if (pending) return;
			if (document.hidden) {
				tracker.clear();
				outlines = [];
			} else {
				const capture = await readFaces(video, false, cameraController?.signal);
				if (token !== generation || !running || !gallery || !currentScanDay()) return;
				const recognized = capture.faces.map((face) => ({
					face,
					id: matchFace(gallery!.faces, face.values, gallery!.configuration)
				}));
				outlines = recognized.map(({ face, id }) => ({
					box: face.box,
					label: id ? (seen.has(id) ? 'เช็คชื่อแล้ว' : 'กำลังยืนยัน') : 'ยังจับคู่ไม่ได้'
				}));
				const observations = recognized
					.filter((item) => item.id && !seen.has(item.id) && Math.abs(item.face.yaw) < 0.25)
					.map((item) => ({ studentId: item.id!, box: item.face.box }));
				const ready = tracker.observe(observations, performance.now());
				if (scanMethod === 'automatic') {
					for (const { face, id } of recognized) {
						if (id && ready.has(id) && !seen.has(id)) {
							await acceptFace(capture, face, id, token);
							if (token !== generation || pending || !running) return;
						}
					}
					if (!capture.faces.length) message = 'ยังไม่เห็นใบหน้าชัดเจน มองเข้ากล้องและเพิ่มแสง';
					else if (!observations.length)
						message = recognized.some((item) => item.id && seen.has(item.id))
							? 'เช็คชื่อแล้ว เชิญคนถัดไป'
							: 'ยังจับคู่ไม่ได้ มองกล้องใหม่หรือให้ครูเช็คชื่อ';
					else if (!ready.size) message = 'เห็นใบหน้าแล้ว กำลังยืนยันต่อเนื่อง';
				} else {
					const single =
						recognized.length === 1 && capture.detectedCount === 1 ? recognized[0] : null;
					const id = single?.id,
						face = single?.face;
					if (!id || !face || seen.has(id)) {
						sampleTarget = 'center';
						lastStudent = '';
						message = 'มองตรงเข้ากล้องทีละคน';
					} else if (sampleTarget === 'center' || id !== lastStudent) {
						if (Math.abs(face.yaw) < 0.08) {
							lastStudent = id;
							sampleTarget = 'turn';
							message = `หันหน้า${challenge}ของคุณเล็กน้อย แล้วกลับมองตรง`;
						}
					} else if (
						sampleTarget === 'turn' &&
						((challenge === 'ซ้าย' && face.yaw > 0.06) || (challenge === 'ขวา' && face.yaw < -0.06))
					) {
						sampleTarget = 'return';
						message = 'กลับมองตรงเพื่อบันทึก';
					} else if (sampleTarget === 'return' && Math.abs(face.yaw) < 0.06) {
						await acceptFace(capture, face, id, token);
						challenge = Math.random() < 0.5 ? 'ซ้าย' : 'ขวา';
					}
				}
				if (capture.detectedCount > 4 && !pending)
					message += ' · เห็นหลายคน กรุณาแบ่งช่องเดินให้เห็นใบหน้าชัด';
			}
		} catch (e) {
			if (token !== generation) return;
			message = e instanceof Error ? e.message : 'สแกนไม่ได้';
			tracker.clear();
			if (pending) {
				message += ' · กดส่งรายการเดิมอีกครั้ง';
				return;
			}
		}
		if (token === generation && running)
			timer = setTimeout(
				() => void tick(token),
				Math.max(document.hidden ? 1000 : 100, 250 - (performance.now() - started))
			);
	}
</script>

<Alert class="border-primary/20 bg-gradient-to-r from-primary/10 to-card">
	<ShieldCheck class="text-primary" /><AlertTitle>เช็คชื่อโดยมีครูดูแล</AlertTitle><AlertDescription
		>จัดกล้องให้เห็นใบหน้าชัดและมีครูดูแล ระบบยืนยันต่อเนื่องได้หลายคน
		กรณีจับคู่ไม่ได้ให้ครูเช็คชื่อด้วยมือ</AlertDescription
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
			<div class="relative overflow-hidden rounded-lg bg-black">
				<video
					bind:this={video}
					class="block w-full rounded-lg bg-black"
					style:aspect-ratio={cameraAspectRatio}
					onloadedmetadata={() => {
						if (video.videoWidth && video.videoHeight)
							cameraAspectRatio = `${video.videoWidth} / ${video.videoHeight}`;
					}}
					autoplay
					muted
					playsinline
					aria-label="ภาพสดจากเว็บแคม"><track kind="captions" /></video
				>
				{#if mode === 'enroll'}
					<div
						class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-3"
						aria-hidden="true"
					>
						<div
							class="flex h-3/5 aspect-[3/4] items-center justify-center rounded-[50%] border-2 border-primary bg-primary/5"
						>
							{#if samples.length === 1}<ArrowRight
									class="size-10 text-primary"
								/>{:else if samples.length === 2}<ArrowLeft
									class="size-10 text-primary"
								/>{:else}<Focus class="size-10 text-primary" />{/if}
						</div>
						<span class="rounded-lg bg-background/90 px-3 py-2 text-sm font-medium">{poseHint}</span
						>
					</div>
				{:else}
					{#each outlines as outline, index (index)}
						<div
							class="pointer-events-none absolute rounded-lg border-2 border-primary"
							style:left={`${outline.box.x * 100}%`}
							style:top={`${outline.box.y * 100}%`}
							style:width={`${outline.box.width * 100}%`}
							style:height={`${outline.box.height * 100}%`}
						>
							<span
								class="absolute -top-6 left-0 whitespace-nowrap rounded bg-background/90 px-1 text-xs"
								>{outline.label}</span
							>
						</div>
					{/each}
				{/if}
			</div>
			<p class="text-xs text-muted-foreground">ภาพไม่กลับด้าน · ลูกศรบอกทิศทางในภาพ</p>
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
					? 'เลือกเครื่องและวิธียืนยันใบหน้าก่อนเริ่มเช็คชื่อ'
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
				<div class="space-y-2">
					<Label for="attendance-scan-method">วิธียืนยันใบหน้า</Label>
					<AttendanceSelect
						id="attendance-scan-method"
						label="วิธียืนยันใบหน้า"
						bind:value={scanMethod}
						disabled={running || busy}
						options={[
							{ value: 'automatic', label: 'อัตโนมัติ · เห็นใบหน้าต่อเนื่อง' },
							{ value: 'gesture', label: 'หันหน้าแล้วมองตรง · ทีละคน' }
						]}
					/>
					<p class="text-xs text-muted-foreground">
						อัตโนมัติสำหรับจุดที่มีครูดูแล ไม่มีการทดสอบการหันหน้า
					</p>
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
						ยืนยันได้สูงสุด 4 ใบหน้าต่อเฟรม บันทึกทีละรายการ เก็บเฉพาะภาพใบหน้าที่ส่งบันทึก
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
						>{samples.length === 3
							? 'บันทึกตัวอย่างอีกครั้ง'
							: `เก็บตัวอย่าง (${samples.length}/3)`}</LoadingButton
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
							removalStudent = student;
							confirmRemoval = true;
						}}
						disabled={!student || !consent || busy || !enrollAllowed}>ถอนการลงทะเบียนใบหน้า</Button
					>
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
{#if enrollAllowed}
	<AttendanceRouteRegion
		initial={enrollmentSource}
		retry={(signal) => attendanceFaceEnrollments(term, { signal })}
		errorTitle="โหลดรายชื่อที่ลงทะเบียนใบหน้าไม่ได้"
		retryLabel="ลองโหลดรายชื่อใหม่"
	>
		{#snippet children(enrollments)}
			<FaceEnrollmentList
				students={initial.students}
				{enrollments}
				disabled={busy}
				onrefresh={reloadEnrollments}
				onremove={(id) => {
					removalStudent = id;
					confirmRemoval = true;
				}}
			/>
		{/snippet}
	</AttendanceRouteRegion>
{/if}
<AlertDialog.Root bind:open={confirmRemoval}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยันถอนการลงทะเบียนใบหน้า</AlertDialog.Title><AlertDialog.Description
				>ลบข้อมูลใบหน้าของ {initial.students.find((s) => s.id === removalStudent)?.name} นักเรียนจะต้องลงทะเบียนและให้ความยินยอมใหม่ก่อนใช้เว็บแคมเช็คชื่อ
				การเช็คชื่อด้วยมือยังใช้งานได้</AlertDialog.Description
			></AlertDialog.Header
		><AlertDialog.Footer
			><AlertDialog.Cancel>ยกเลิก</AlertDialog.Cancel><AlertDialog.Action
				variant="destructive"
				onclick={remove}
				disabled={!removalStudent || busy || !enrollAllowed}
				>ยืนยันถอนข้อมูลใบหน้า</AlertDialog.Action
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
