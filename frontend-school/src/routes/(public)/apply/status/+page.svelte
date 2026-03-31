<script lang="ts">
	import {
		portalCheck,
		portalGetStatus,
		portalGetExamSeat,
		portalSubmitForm
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { getPublicSchoolInfo, type PublicSchoolInfo } from '$lib/api/school';
	import {
		GraduationCap,
		Search,
		Check,
		FileText,
		AlertCircle,
		Clock,
		X,
		Edit3,
		ArrowLeft,
		Plus,
		Trash2,
		Copy
	} from 'lucide-svelte';

	type PortalStep = 'login' | 'status';

	let schoolInfo = $state<PublicSchoolInfo>({});

	onMount(async () => {
		schoolInfo = await getPublicSchoolInfo();
	});

	let step: PortalStep = $state('login');
	let checking = $state(false);
	let savingForm = $state(false);

	let nationalId = $state(''); // raw: digits or alphanumeric (e.g. G-code)
	let dateOfBirth = $state(''); // format: DDMMYYYY พ.ศ.

	// ถ้าเป็นตัวเลขล้วน 13 หลัก → format X-XXXX-XXXXX-XX-X
	// ถ้ามีตัวอักษร → แสดงตามที่กรอก (ไม่ format)
	const nationalIdDisplay = $derived(() => {
		const d = nationalId;
		const isAllDigits = /^\d+$/.test(d);
		if (!isAllDigits) return d;
		if (d.length <= 1) return d;
		if (d.length <= 5) return d[0] + '-' + d.slice(1);
		if (d.length <= 10) return d[0] + '-' + d.slice(1, 5) + '-' + d.slice(5);
		if (d.length <= 12) return d[0] + '-' + d.slice(1, 5) + '-' + d.slice(5, 10) + '-' + d.slice(10);
		return d[0] + '-' + d.slice(1, 5) + '-' + d.slice(5, 10) + '-' + d.slice(10, 12) + '-' + d[12];
	});

	function handleNationalIdInput(e: Event) {
		const raw = (e.target as HTMLInputElement).value.replace(/[^a-zA-Z0-9]/g, '').slice(0, 13);
		nationalId = raw;
	}

	function goToEdit() {
		if (portalData?.application?.admissionRoundId) {
			sessionStorage.setItem('admissionEditNid', nationalId);
			sessionStorage.setItem('admissionEditDob', dateOfBirth);
			goto(`/apply/${portalData.application.admissionRoundId}?edit=true`);
		}
	}

	let examSeat: {
		seatNumber: number;
		examId?: string;
		roomName: string;
		buildingName?: string;
		examDate?: string;
	} | null = $state(null);

	let portalData: {
		roundStatus?: string;
		assignmentMode?: string;
		application?: {
			id: string;
			admissionRoundId: string;
			applicationNumber?: string;
			title?: string;
			firstName: string;
			lastName: string;
			status: string;
			dateOfBirth?: string;
			trackName?: string;
			assignedTrackName?: string;
			roundName?: string;
			rejectionReason?: string;
			fatherName?: string;
			fatherPhone?: string;
			motherName?: string;
			motherPhone?: string;
			guardianName?: string;
			guardianPhone?: string;
			guardianRelation?: string;
			guardianIs?: string;
		};
		assignment?: {
			rankInTrack?: number;
			rankInRoom?: number;
			totalScore?: number;
			roomName?: string;
			studentConfirmed: boolean;
		} | null;
		scores?: { subjectName?: string; score?: number; maxScore?: number }[] | null;
		enrollmentForm?: { formData: Record<string, unknown>; preSubmittedAt?: string } | null;
	} | null = $state(null);

	// ช่วง round ที่ถือว่า "ประกาศผลแล้ว" — ก่อนหน้านี้ไม่ควรโชว์ "ผ่านการคัดเลือก"
	const RESULT_ANNOUNCED_STATUSES = ['announced', 'enrolling', 'closed'];

	// effective status สำหรับแสดงผล
	// ถ้า app.status = 'accepted' แต่ round ยังไม่ประกาศ → แสดงเป็น 'pending_result' แทน
	const effectiveStatus = $derived(() => {
		const rs = portalData?.roundStatus ?? '';
		const as = portalData?.application?.status ?? '';
		if ((as === 'accepted' || as === 'scored') && !RESULT_ANNOUNCED_STATUSES.includes(rs)) {
			return 'pending_result';
		}
		return as;
	});

	const statusLabel: Record<string, string> = {
		submitted: 'รอตรวจสอบเอกสาร',
		verified: 'เอกสารผ่านแล้ว รอวันสอบ',
		scored: 'รอประกาศผลการคัดเลือก',
		pending_result: 'รอประกาศผลการคัดเลือก',
		absent: 'ขาดสอบ',
		rejected: 'ไม่ผ่านการคัดเลือก',
		accepted: 'ผ่านการคัดเลือก!',
		enrolled: 'มอบตัวสำเร็จ',
		withdrawn: 'ถอนตัวแล้ว'
	};

	const statusColor: Record<string, string> = {
		submitted: 'bg-yellow-50 border-yellow-200 text-yellow-800',
		verified: 'bg-blue-50 border-blue-200 text-blue-800',
		scored: 'bg-blue-50 border-blue-200 text-blue-800',
		pending_result: 'bg-blue-50 border-blue-200 text-blue-800',
		absent: 'bg-gray-50 border-gray-200 text-gray-800',
		rejected: 'bg-red-50 border-red-200 text-red-800',
		accepted: 'bg-green-50 border-green-200 text-green-800',
		enrolled: 'bg-purple-50 border-purple-200 text-purple-800',
		withdrawn: 'bg-gray-50 border-gray-200 text-gray-800'
	};

	// Enrollment form fields
	interface ParentEntry {
		title: string;
		firstName: string;
		lastName: string;
		phone: string;
		relationship: string;
	}

	let formFields = $state({
		bloodType: '',
		medicalConditions: '',
		allergies: '',
		father: { title: 'นาย', firstName: '', lastName: '', phone: '' },
		mother: { title: 'นาง', firstName: '', lastName: '', phone: '' },
		guardians: [] as ParentEntry[]
	});

	function addGuardian() {
		formFields.guardians = [...formFields.guardians, { title: '', firstName: '', lastName: '', phone: '', relationship: '' }];
	}

	function removeGuardian(index: number) {
		formFields.guardians = formFields.guardians.filter((_, i) => i !== index);
	}

	function copyFromFather(index: number) {
		const g = formFields.guardians[index];
		g.title = formFields.father.title;
		g.firstName = formFields.father.firstName;
		g.lastName = formFields.father.lastName;
		g.phone = formFields.father.phone;
		g.relationship = 'บิดา';
		formFields.guardians = [...formFields.guardians];
	}

	function copyFromMother(index: number) {
		const g = formFields.guardians[index];
		g.title = formFields.mother.title;
		g.firstName = formFields.mother.firstName;
		g.lastName = formFields.mother.lastName;
		g.phone = formFields.mother.phone;
		g.relationship = 'มารดา';
		formFields.guardians = [...formFields.guardians];
	}

	async function handleCheck(e: Event) {
		e.preventDefault();
		const dob = dateOfBirth.trim();
		if (!nationalId.trim()) {
			toast.error('กรุณากรอกเลขบัตรประชาชนหรือรหัสผู้สมัคร');
			return;
		}
		if (dob.length !== 8) {
			toast.error('กรุณากรอกวันเกิดให้ครบ 8 หลัก (ววดดปปปป)');
			return;
		}
		checking = true;
		try {
			await portalCheck(nationalId, dob);
			await loadStatus();
			step = 'status';
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ข้อมูลไม่ถูกต้อง');
		} finally {
			checking = false;
		}
	}

	async function loadStatus() {
		try {
			portalData = (await portalGetStatus(nationalId, dateOfBirth.trim())) as typeof portalData;
			const app = portalData?.application;
			if (portalData?.enrollmentForm?.formData) {
				// โหลดข้อมูลจากฟอร์มที่บันทึกไว้
				const fd = portalData.enrollmentForm.formData as Record<string, unknown>;
				formFields.bloodType = (fd.bloodType as string) ?? '';
				formFields.medicalConditions = (fd.medicalConditions as string) ?? '';
				formFields.allergies = (fd.allergies as string) ?? '';
				if (fd.father) {
					const f = fd.father as Record<string, string>;
					formFields.father = { title: f.title ?? 'นาย', firstName: f.firstName ?? '', lastName: f.lastName ?? '', phone: f.phone ?? '' };
				}
				if (fd.mother) {
					const m = fd.mother as Record<string, string>;
					formFields.mother = { title: m.title ?? 'นาง', firstName: m.firstName ?? '', lastName: m.lastName ?? '', phone: m.phone ?? '' };
				}
				if (fd.guardians && Array.isArray(fd.guardians)) {
					formFields.guardians = (fd.guardians as ParentEntry[]).map(g => ({
						title: g.title ?? '', firstName: g.firstName ?? '', lastName: g.lastName ?? '',
						phone: g.phone ?? '', relationship: g.relationship ?? ''
					}));
				}
			} else if (app) {
				// Pre-fill จากข้อมูลที่กรอกตอนสมัคร
				if (app.fatherName) {
					const parts = app.fatherName.split(' ');
					formFields.father.firstName = parts[0] ?? '';
					formFields.father.lastName = parts.slice(1).join(' ') ?? '';
				}
				if (app.fatherPhone) formFields.father.phone = app.fatherPhone;
				if (app.motherName) {
					const parts = app.motherName.split(' ');
					formFields.mother.firstName = parts[0] ?? '';
					formFields.mother.lastName = parts.slice(1).join(' ') ?? '';
				}
				if (app.motherPhone) formFields.mother.phone = app.motherPhone;
				// ถ้ามีข้อมูลผู้ปกครอง pre-fill เป็น guardian ตัวแรก
				if (app.guardianName && app.guardianPhone) {
					const parts = app.guardianName.split(' ');
					formFields.guardians = [{
						title: '',
						firstName: parts[0] ?? '',
						lastName: parts.slice(1).join(' ') ?? '',
						phone: app.guardianPhone,
						relationship: app.guardianRelation ?? ''
					}];
				}
			}

			// โหลดที่นั่งสอบเฉพาะช่วงประกาศห้องสอบ
			const rs = portalData?.roundStatus;
			if (rs === 'exam_announced') {
				try {
					examSeat = await portalGetExamSeat(nationalId, dateOfBirth.trim());
				} catch {
					examSeat = null;
				}
			} else {
				examSeat = null;
			}
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		}
	}

	async function handleSubmitForm(e: Event) {
		e.preventDefault();
		// ต้องมีผู้ปกครองอย่างน้อย 1 คน
		if (formFields.guardians.length === 0) {
			toast.error('กรุณาเพิ่มผู้ปกครองอย่างน้อย 1 คน');
			return;
		}
		const hasEmptyGuardian = formFields.guardians.some(g => !g.firstName.trim() || !g.phone.trim());
		if (hasEmptyGuardian) {
			toast.error('กรุณากรอกชื่อและเบอร์โทรผู้ปกครองให้ครบ');
			return;
		}
		savingForm = true;
		try {
			await portalSubmitForm(nationalId, dateOfBirth.trim(), formFields as Record<string, unknown>);
			toast.success('ยืนยันมอบตัวและบันทึกข้อมูลสำเร็จ');
			await loadStatus();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingForm = false;
		}
	}
</script>

<svelte:head>
	<title>ตรวจสอบผลการสมัคร - SchoolOrbit</title>
</svelte:head>

<div
	class="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 flex items-start justify-center py-12 px-4"
>
	<div class="w-full max-w-lg space-y-6">
		<!-- Header -->
		<div class="relative flex flex-col items-center">
			<div class="absolute left-0 top-0">
				<Button
					href="/apply"
					variant="ghost"
					size="sm"
					class="text-blue-600 hover:text-blue-700 hover:bg-blue-50 -ml-2"
				>
					<ArrowLeft class="w-4 h-4" />
				</Button>
			</div>
			{#if schoolInfo.logoUrl}
				<img src={schoolInfo.logoUrl} alt="school logo" class="w-24 h-24 object-contain mb-4" />
			{:else}
				<div class="inline-flex p-3 bg-white rounded-2xl shadow-md mb-4">
					<GraduationCap class="w-10 h-10 text-blue-600" />
				</div>
			{/if}
			<h1 class="text-2xl font-bold text-gray-900">ตรวจสอบผลการสมัครเรียน</h1>
			<p class="text-gray-500 mt-1 text-sm">กรอกเลขบัตรประชาชนและวันเดือนปีเกิดเพื่อตรวจสอบผล</p>
		</div>

		{#if step === 'login'}
			<!-- Login Form -->

			<div class="bg-white rounded-2xl shadow-lg p-6">
				<form onsubmit={handleCheck} class="space-y-4">
					<div class="space-y-1.5">
						<label for="national-id" class="text-sm font-medium text-gray-700"
							>เลขบัตรประชาชน หรือ รหัสผู้สมัคร</label
						>
						<Input
							id="national-id"
							value={nationalIdDisplay()}
							oninput={handleNationalIdInput}
							maxlength={17}
							placeholder="เลขบัตร 13 หลัก หรือ รหัส G"
							class="h-11 tracking-widest"
						/>
					</div>
					<div class="space-y-1.5">
						<label for="dob" class="text-sm font-medium text-gray-700"
							>วันเดือนปีเกิด <span class="text-xs text-muted-foreground">(พ.ศ.)</span></label
						>
						<Input
							id="dob"
							type="text"
							inputmode="numeric"
							maxlength={8}
							placeholder="เช่น 20082543 (ววดดปปปป 8 หลัก)"
							bind:value={dateOfBirth}
							class="h-11 text-center"
						/>
					</div>
					<Button
						type="submit"
						disabled={checking}
						class="w-full h-11 gap-2 bg-blue-600 hover:bg-blue-700"
					>
						<Search class="w-4 h-4" />
						{checking ? 'กำลังตรวจสอบ...' : 'ตรวจสอบผลการสมัคร'}
					</Button>
				</form>
			</div>
		{:else if step === 'status' && portalData}
			{@const app = portalData.application}
			{@const assignment = portalData.assignment}
			{@const scores = portalData.scores}
			{@const form = portalData.enrollmentForm}
			{@const roundStatus = portalData.roundStatus}

			<!-- Status Card -->
			{#if app}
				<div class="bg-white rounded-2xl shadow-lg p-6 space-y-4">
					<div class="flex items-start justify-between">
						<div>
							<p class="text-xs text-gray-400 uppercase tracking-wide">ผู้สมัคร</p>
							<p class="text-xl font-bold text-gray-900">{(app.title ?? '') + app.firstName} {app.lastName}</p>
							<p class="text-sm text-gray-500">
								ใบสมัคร: <span class="font-mono font-semibold">{app.applicationNumber}</span>
							</p>
							<p class="text-sm text-gray-500">
								สาย: {app.trackName ?? '-'} | รอบ: {app.roundName ?? '-'}
							</p>
						</div>
					</div>

					<div
						class="border rounded-xl p-4 {statusColor[effectiveStatus()] ?? 'bg-gray-50 border-gray-200'}"
					>
						<div class="flex flex-col gap-2">
							<div class="flex items-center gap-2">
								{#if effectiveStatus() === 'accepted' || effectiveStatus() === 'enrolled'}
									<Check class="w-5 h-5" />
								{:else if effectiveStatus() === 'rejected'}
									<X class="w-5 h-5" />
								{:else}
									<Clock class="w-5 h-5" />
								{/if}
								<p class="font-semibold">{statusLabel[effectiveStatus()] ?? app.status}</p>
							</div>

							<!-- แสดงเหตุผลถ้าถูกปฏิเสธ -->
							{#if app.status === 'rejected' && app.rejectionReason}
								<div
									class="mt-2 p-3 bg-red-50/50 border border-red-100 rounded-lg text-sm text-red-800"
								>
									<strong>หมายเหตุ:</strong>
									{app.rejectionReason}
								</div>

								<div class="mt-4 pt-4 border-t border-red-100">
									<p class="text-xs text-gray-500 mb-3">
										คุณสามารถแก้ไขใบสมัครตามเหตุผลข้างต้นโดยไม่ต้องกรอกข้อมูลใหม่ทั้งหมด
									</p>
									<Button
										onclick={goToEdit}
										variant="outline"
										class="w-full text-blue-600 border-blue-200 hover:bg-blue-50"
									>
										<Edit3 class="w-4 h-4 mr-2" />
										แก้ไขข้อมูลใบสมัคร
									</Button>
								</div>
							{/if}
						</div>
					</div>

					<!-- Result (if accepted) -->
					{#if assignment && (effectiveStatus() === 'accepted' || effectiveStatus() === 'enrolled')}
						<div class="border border-green-200 bg-green-50 rounded-xl p-4 space-y-2">
							<p class="font-semibold text-green-800 flex items-center gap-2">
								<GraduationCap class="w-4 h-4" />
								ผลการคัดเลือก
							</p>
							{#if app?.assignedTrackName}
								<p class="text-sm text-green-700">สายการเรียน: <span class="font-semibold">{app.assignedTrackName}</span></p>
							{:else if app?.trackName}
								<p class="text-sm text-green-700">สายการเรียน: <span class="font-semibold">{app.trackName}</span></p>
							{/if}
							<div class="grid grid-cols-3 gap-3 text-center">
								<div>
									<p class="text-2xl font-bold text-green-700">{assignment.rankInTrack ?? '-'}</p>
									<p class="text-xs text-green-600">{portalData?.assignmentMode === 'global' ? 'อันดับรวม' : 'อันดับในสาย'}</p>
								</div>
								<div>
									<p class="text-2xl font-bold text-green-700">
										{assignment.totalScore?.toFixed(1) ?? '-'}
									</p>
									<p class="text-xs text-green-600">คะแนนรวม</p>
								</div>
								<div>
									<p class="text-xl font-bold text-green-700">{assignment.roomName ?? '-'}</p>
									<p class="text-xs text-green-600">ห้องเรียน</p>
								</div>
							</div>
						</div>

						{#if assignment.studentConfirmed}
							<div
								class="flex items-center gap-2 px-3 py-2 bg-green-100 text-green-700 rounded-lg text-sm"
							>
								<Check class="w-4 h-4" />
								ยืนยันมอบตัวแล้ว
							</div>
						{/if}
					{/if}

					<!-- Exam Seat -->
					{#if examSeat && effectiveStatus() !== 'absent'}
						<div class="border border-blue-200 bg-blue-50 rounded-xl p-4 space-y-2">
							<p class="font-semibold text-blue-800 flex items-center gap-2">
								<FileText class="w-4 h-4" /> ที่นั่งสอบ
							</p>
							<div class="grid grid-cols-3 gap-3 text-center">
								<div>
									<p class="text-2xl font-bold text-blue-700">{examSeat.seatNumber}</p>
									<p class="text-xs text-blue-600">เลขที่นั่ง</p>
								</div>
								<div>
									<p class="text-2xl font-bold text-blue-700">{examSeat.roomName}</p>
									<p class="text-xs text-blue-600">ห้องสอบ</p>
								</div>
								<div>
									<p class="text-2xl font-bold text-blue-700">{examSeat.buildingName ?? '-'}</p>
									<p class="text-xs text-blue-600">อาคาร</p>
								</div>
							</div>
							{#if examSeat.examDate}
								<p class="text-xs text-blue-600 text-center pt-1">
									วันสอบ: {new Date(examSeat.examDate).toLocaleDateString('th-TH', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' })}
								</p>
							{/if}
							{#if examSeat.examId}
								<p class="text-xs text-blue-500 text-center">เลขประจำตัวสอบ: {examSeat.examId}</p>
							{/if}
						</div>
					{/if}

					<!-- Scores — แสดงเฉพาะหลังประกาศผลแล้ว -->
					{#if scores && scores.length > 0 && scores.some((s) => s.score !== null) && RESULT_ANNOUNCED_STATUSES.includes(roundStatus ?? '')}
						<div class="space-y-2">
							<p class="font-medium text-sm text-gray-700 flex items-center gap-2">
								<FileText class="w-4 h-4" /> คะแนนสอบ
							</p>
							<div
								class="divide-y divide-gray-100 border border-gray-200 rounded-lg overflow-hidden"
							>
								{#each scores as s, i}
									<div
										class="flex items-center justify-between px-4 py-2 {i % 2 === 0
											? 'bg-gray-50'
											: 'bg-white'}"
									>
										<span class="text-sm text-gray-700">{s.subjectName}</span>
										<span class="font-semibold text-gray-900">
											{s.score != null ? s.score : '-'}
											<span class="text-xs text-gray-400">/{s.maxScore}</span>
										</span>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</div>
			{/if}

			<!-- Enrollment Form — แสดงเมื่อ accepted + enrolling (ไม่ต้อง confirm ก่อน) -->
			{#if app?.status === 'accepted' && roundStatus === 'enrolling'}
				<div class="bg-white rounded-2xl shadow-lg p-6">
					<h2 class="font-semibold text-gray-900 flex items-center gap-2 mb-4">
						<FileText class="w-5 h-5 text-blue-600" />
						แบบฟอร์มยืนยันมอบตัว
					</h2>
					{#if form}
						<p class="text-xs text-green-600 mb-3">บันทึกแล้ว — สามารถแก้ไขได้</p>
					{:else}
						<p class="text-xs text-orange-600 mb-3">
							<AlertCircle class="w-3 h-3 inline mr-0.5" />
							กรุณากรอกข้อมูลและกดยืนยันมอบตัวภายในกำหนด
						</p>
					{/if}

					<!-- ข้อมูลนักเรียน (read-only) -->
					<div class="border border-gray-200 rounded-lg p-3 mb-4 bg-gray-50 space-y-2">
						<p class="text-xs font-medium text-gray-500 uppercase tracking-wide">ข้อมูลนักเรียน</p>
						<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
							<div>
								<span class="text-gray-500">ชื่อ:</span>
								<span class="font-medium">{(app.title ?? '') + app.firstName} {app.lastName}</span>
							</div>
							<div>
								<span class="text-gray-500">สาย:</span>
								<span class="font-medium">{app.assignedTrackName ?? app.trackName ?? '-'}</span>
							</div>
							{#if assignment?.roomName}
								<div>
									<span class="text-gray-500">ห้อง:</span>
									<span class="font-medium">{assignment.roomName}</span>
								</div>
							{/if}
							{#if app.dateOfBirth}
								<div>
									<span class="text-gray-500">วันเกิด:</span>
									<span class="font-medium">{new Date(app.dateOfBirth).toLocaleDateString('th-TH', { year: 'numeric', month: 'long', day: 'numeric' })}</span>
								</div>
							{/if}
						</div>
					</div>

					<form onsubmit={handleSubmitForm} class="space-y-4">
						<!-- ข้อมูลสุขภาพ -->
						<div class="space-y-3">
							<p class="text-sm font-medium text-gray-700">ข้อมูลสุขภาพ</p>
							<div class="space-y-1">
								<Label class="text-xs font-medium text-gray-600">กลุ่มเลือด</Label>
								<Select.Root type="single" bind:value={formFields.bloodType}>
									<Select.Trigger class="h-8 text-sm">
										{formFields.bloodType || '-- เลือก --'}
									</Select.Trigger>
									<Select.Content>
										{#each ['A', 'B', 'AB', 'O'] as b}
											<Select.Item value={b}>{b}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-1">
								<Label class="text-xs font-medium text-gray-600">โรคประจำตัว</Label>
								<Textarea
									bind:value={formFields.medicalConditions}
									rows={2}
									class="text-sm resize-none"
									placeholder="ถ้าไม่มีใส่ ไม่มี"
								/>
							</div>
							<div class="space-y-1">
								<Label class="text-xs font-medium text-gray-600">แพ้ยา / แพ้อาหาร</Label>
								<Textarea
									bind:value={formFields.allergies}
									rows={2}
									class="text-sm resize-none"
									placeholder="ถ้าไม่มีใส่ ไม่มี"
								/>
							</div>
						</div>

						<!-- ข้อมูลบิดา -->
						<div class="border border-gray-200 rounded-lg p-3 space-y-2">
							<p class="text-sm font-medium text-gray-700">ข้อมูลบิดา</p>
							<div class="grid grid-cols-3 gap-2">
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">คำนำหน้า</Label>
									<Select.Root type="single" bind:value={formFields.father.title}>
										<Select.Trigger class="h-8 text-sm">
											{formFields.father.title || '-- เลือก --'}
										</Select.Trigger>
										<Select.Content>
											{#each ['นาย'] as t}
												<Select.Item value={t}>{t}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">ชื่อ</Label>
									<Input bind:value={formFields.father.firstName} class="h-8 text-sm" placeholder="ชื่อ" />
								</div>
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">นามสกุล</Label>
									<Input bind:value={formFields.father.lastName} class="h-8 text-sm" placeholder="นามสกุล" />
								</div>
							</div>
							<div class="space-y-1">
								<Label class="text-xs text-gray-600">เบอร์โทร</Label>
								<Input bind:value={formFields.father.phone} class="h-8 text-sm" placeholder="0XX-XXX-XXXX" />
							</div>
						</div>

						<!-- ข้อมูลมารดา -->
						<div class="border border-gray-200 rounded-lg p-3 space-y-2">
							<p class="text-sm font-medium text-gray-700">ข้อมูลมารดา</p>
							<div class="grid grid-cols-3 gap-2">
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">คำนำหน้า</Label>
									<Select.Root type="single" bind:value={formFields.mother.title}>
										<Select.Trigger class="h-8 text-sm">
											{formFields.mother.title || '-- เลือก --'}
										</Select.Trigger>
										<Select.Content>
											{#each ['นาง', 'นางสาว'] as t}
												<Select.Item value={t}>{t}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">ชื่อ</Label>
									<Input bind:value={formFields.mother.firstName} class="h-8 text-sm" placeholder="ชื่อ" />
								</div>
								<div class="space-y-1">
									<Label class="text-xs text-gray-600">นามสกุล</Label>
									<Input bind:value={formFields.mother.lastName} class="h-8 text-sm" placeholder="นามสกุล" />
								</div>
							</div>
							<div class="space-y-1">
								<Label class="text-xs text-gray-600">เบอร์โทร</Label>
								<Input bind:value={formFields.mother.phone} class="h-8 text-sm" placeholder="0XX-XXX-XXXX" />
							</div>
						</div>

						<!-- ผู้ปกครอง -->
						<div class="space-y-3">
							<div class="flex items-center justify-between">
								<p class="text-sm font-medium text-gray-700">ผู้ปกครอง <span class="text-red-500">*</span></p>
								<Button type="button" variant="outline" size="sm" onclick={addGuardian} class="h-7 text-xs gap-1">
									<Plus class="w-3 h-3" /> เพิ่มผู้ปกครอง
								</Button>
							</div>
							{#if formFields.guardians.length === 0}
								<p class="text-xs text-gray-400 text-center py-3 border border-dashed border-gray-200 rounded-lg">
									กรุณาเพิ่มผู้ปกครองอย่างน้อย 1 คน
								</p>
							{/if}
							{#each formFields.guardians as guardian, i}
								<div class="border border-blue-200 bg-blue-50/30 rounded-lg p-3 space-y-2">
									<div class="flex items-center justify-between">
										<p class="text-xs font-medium text-blue-700">ผู้ปกครองคนที่ {i + 1}</p>
										<div class="flex gap-1">
											<Button type="button" variant="ghost" size="sm" onclick={() => copyFromFather(i)} class="h-6 text-xs px-2 text-gray-500 hover:text-blue-600" title="ใช้ข้อมูลจากบิดา">
												<Copy class="w-3 h-3 mr-1" /> บิดา
											</Button>
											<Button type="button" variant="ghost" size="sm" onclick={() => copyFromMother(i)} class="h-6 text-xs px-2 text-gray-500 hover:text-blue-600" title="ใช้ข้อมูลจากมารดา">
												<Copy class="w-3 h-3 mr-1" /> มารดา
											</Button>
											<Button type="button" variant="ghost" size="sm" onclick={() => removeGuardian(i)} class="h-6 px-1 text-red-400 hover:text-red-600">
												<Trash2 class="w-3 h-3" />
											</Button>
										</div>
									</div>
									<div class="grid grid-cols-3 gap-2">
										<div class="space-y-1">
											<Label class="text-xs text-gray-600">คำนำหน้า</Label>
											<Select.Root type="single" bind:value={guardian.title}>
												<Select.Trigger class="h-8 text-sm">
													{guardian.title || '-- เลือก --'}
												</Select.Trigger>
												<Select.Content>
													{#each ['นาย', 'นาง', 'นางสาว'] as t}
														<Select.Item value={t}>{t}</Select.Item>
													{/each}
												</Select.Content>
											</Select.Root>
										</div>
										<div class="space-y-1">
											<Label class="text-xs text-gray-600">ชื่อ <span class="text-red-500">*</span></Label>
											<Input bind:value={guardian.firstName} class="h-8 text-sm" placeholder="ชื่อ" />
										</div>
										<div class="space-y-1">
											<Label class="text-xs text-gray-600">นามสกุล</Label>
											<Input bind:value={guardian.lastName} class="h-8 text-sm" placeholder="นามสกุล" />
										</div>
									</div>
									<div class="grid grid-cols-2 gap-2">
										<div class="space-y-1">
											<Label class="text-xs text-gray-600">เบอร์โทร <span class="text-red-500">*</span></Label>
											<Input bind:value={guardian.phone} class="h-8 text-sm" placeholder="0XX-XXX-XXXX" />
										</div>
										<div class="space-y-1">
											<Label class="text-xs text-gray-600">ความสัมพันธ์</Label>
											<Select.Root type="single" bind:value={guardian.relationship}>
												<Select.Trigger class="h-8 text-sm">
													{guardian.relationship || '-- เลือก --'}
												</Select.Trigger>
												<Select.Content>
													{#each ['บิดา', 'มารดา', 'ปู่', 'ย่า', 'ตา', 'ยาย', 'ลุง', 'ป้า', 'น้า', 'อา', 'พี่', 'อื่นๆ'] as r}
														<Select.Item value={r}>{r}</Select.Item>
													{/each}
												</Select.Content>
											</Select.Root>
										</div>
									</div>
								</div>
							{/each}
						</div>

						<Button
							type="submit"
							disabled={savingForm}
							class="w-full gap-2 text-white bg-green-600 hover:bg-green-700"
						>
							<Check class="w-4 h-4" />
							{savingForm ? 'กำลังบันทึก...' : form ? 'อัปเดตข้อมูลมอบตัว' : 'ยืนยันมอบตัว'}
						</Button>
					</form>
				</div>
			{/if}
		{/if}
	</div>
</div>
