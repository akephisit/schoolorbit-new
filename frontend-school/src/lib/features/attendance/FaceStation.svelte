<script lang="ts">
	import AttendanceSelect from './AttendanceSelect.svelte';
	import { onDestroy } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { uploadFile, deleteFile } from '#lib/api/files.js';
	import {
		enrollAttendanceFace,
		removeAttendanceFace,
		openAttendanceKiosk,
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
	} from './face-camera';
	let { term, date, initial }: { term: string; date: string; initial: AttendanceOptions } =
		$props();
	let video: HTMLVideoElement,
		stream: MediaStream | null = null,
		device = $state(''),
		student = $state(''),
		consent = $state(false),
		active = $state(false),
		busy = $state(false),
		running = $state(false),
		message = $state('เปิดเว็บแคมเพื่อเริ่มต้น'),
		gallery = $state<AttendanceKioskWorkspace | null>(null),
		samples = $state<number[][]>([]),
		mode = $state<'scan' | 'enroll'>('scan'),
		sampleTarget = $state<'center' | 'turn' | 'return'>('center'),
		challenge = $state(''),
		lastStudent = $state('');
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
			toast.error(message);
		} finally {
			if (token === generation) busy = false;
		}
	}
	function stop() {
		generation++;
		busy = false;
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
			toast.error('เลือกนักเรียนและยืนยันการลงทะเบียน');
			return;
		}
		busy = true;
		try {
			const face = await readFace(video);
			if (!face) throw new Error('ต้องเห็นใบหน้าชัดเจนเพียงคนเดียว');
			if (samples.length && Math.abs(face.yaw) < 0.04)
				throw new Error('หันหน้าซ้ายหรือขวาเล็กน้อยเพื่อเก็บมุมเพิ่ม');
			if (samples.length && distance(samples[0], face.values) > 0.55)
				throw new Error('ใบหน้าเปลี่ยนไป กรุณาเริ่มเก็บใหม่');
			samples = [...samples, face.values];
			message = `เก็บ ${samples.length}/3 ตัวอย่างแล้ว`;
			if (samples.length >= 3) {
				await enrollAttendanceFace(student, {
					model: FACE_MODEL,
					descriptors: samples.map((values) => ({ values })),
					consentConfirmed: consent
				});
				toast.success('ลงทะเบียนใบหน้าแล้ว');
				samples = [];
				consent = false;
			}
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เก็บใบหน้าไม่ได้');
		} finally {
			busy = false;
		}
	}
	async function remove() {
		if (!student || !consent) return;
		busy = true;
		try {
			await removeAttendanceFace(student);
			toast.success('ลบข้อมูลใบหน้าแล้ว');
			samples = [];
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่ได้');
		} finally {
			busy = false;
		}
	}
	async function beginScanning() {
		const token = ++generation;
		busy = true;
		try {
			const opened = await openAttendanceKiosk({ academicTermId: term, date, deviceId: device });
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
			toast.error(e instanceof Error ? e.message : 'เปิดเครื่องสแกนไม่ได้');
			running = false;
		} finally {
			if (token === generation) busy = false;
		}
	}
	async function commit() {
		if (!pending) return;
		const queued = pending;
		const result = await scanAttendance(queued.payload);
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
		if (token !== generation || !running || !gallery) return;
		try {
			if (pending) return;
			const face = await readFace(video);
			if (token !== generation) return;
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
						if (token !== generation) return;
						const file = await uploadFile(image, 'attendance_evidence', id);
						if (token !== generation) {
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

<Button variant="outline" href={`/staff/attendance?academicTermId=${term}`}>กลับหน้าเช็คชื่อ</Button
>
<p class="rounded-lg bg-muted p-4 text-sm">
	ใช้คอมพิวเตอร์กับเว็บแคมให้เห็นทีละคน และมีครูดูแล กรณีจับคู่ไม่ได้ให้ครูเช็คชื่อด้วยมือ
	การหันหน้าช่วยตรวจภาพนิ่ง แต่ยังต้องทดสอบแสงและความแม่นยำก่อนใช้เป็นหลัก
</p>
<div class="flex gap-2">
	{#if scanAllowed}<Button
			variant={mode === 'scan' ? 'default' : 'outline'}
			disabled={running}
			onclick={() => {
				mode = 'scan';
				samples = [];
			}}>สแกนเข้าโรงเรียน</Button
		>{/if}{#if enrollAllowed}<Button
			variant={mode === 'enroll' ? 'default' : 'outline'}
			disabled={running}
			onclick={() => {
				mode = 'enroll';
			}}>ลงทะเบียนใบหน้า</Button
		>{/if}
</div>
<div class="grid gap-5 lg:grid-cols-2">
	<section class="space-y-3">
		<video
			bind:this={video}
			class="aspect-[4/3] w-full rounded-xl bg-black"
			autoplay
			muted
			playsinline
			aria-label="ภาพสดจากเว็บแคม"><track kind="captions" /></video
		>
		<p role="status" aria-live="polite" class="rounded-lg border p-4 font-medium">{message}</p>
		<div class="flex gap-2">
			<Button onclick={start} disabled={busy || active}>เปิดเว็บแคม</Button><Button
				variant="outline"
				onclick={stop}
				disabled={!active}>หยุดและปิดกล้อง</Button
			>{#if pending}<Button onclick={retry} disabled={busy}>ส่งรายการเดิมอีกครั้ง</Button>{/if}
		</div>
	</section>
	<section class="space-y-4">
		{#if mode === 'scan'}<label class="block"
				>เครื่องสแกน<AttendanceSelect
					label="เครื่องสแกน"
					placeholder="เลือกเครื่องที่ได้รับมอบหมาย"
					bind:value={device}
					disabled={running || busy}
					options={initial.devices
						.filter((d) => d.enabled)
						.map((d) => ({ value: d.id, label: d.name }))}
				/></label
			><Button
				onclick={beginScanning}
				disabled={!active || !device || busy || running || !scanAllowed}>เริ่มเช็คชื่อวันนี้</Button
			>
			<p class="text-sm text-muted-foreground">
				ทำงานทีละคน เก็บภาพเฉพาะรายการที่ส่งบันทึก แจ้งนักเรียนและผู้ปกครองทุกคนที่เชื่อมกับนักเรียน
				เมื่อหยุดกล้อง ข้อมูลใบหน้าในหน้านี้จะถูกล้าง
			</p>{:else}<label class="block"
				>นักเรียน<AttendanceSelect
					label="นักเรียน"
					placeholder="เลือกนักเรียน"
					bind:value={student}
					onValueChange={() => {
						samples = [];
					}}
					options={initial.students.map((s) => ({
						value: s.id,
						label: `${s.homeroomName ?? '-'} · ${s.name}`
					}))}
				/></label
			><label class="flex gap-2"
				><input
					type="checkbox"
					bind:checked={consent}
				/>ยืนยันว่าได้รับความยินยอมและตรวจว่าเป็นนักเรียนคนที่เลือก</label
			>
			<p>เก็บ 3 ตัวอย่าง: หน้าตรง แล้วหันซ้ายและขวาเล็กน้อย</p>
			<Button onclick={collect} disabled={!active || !student || !consent || busy || !enrollAllowed}
				>เก็บตัวอย่าง ({samples.length}/3)</Button
			><Button
				variant="outline"
				onclick={() => {
					samples = [];
				}}>เริ่มเก็บใหม่</Button
			><Button
				variant="destructive"
				onclick={remove}
				disabled={!student || !consent || busy || !enrollAllowed}>ถอนการลงทะเบียนใบหน้า</Button
			>{/if}
	</section>
</div>
