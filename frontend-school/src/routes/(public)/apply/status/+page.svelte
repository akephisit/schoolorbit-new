<script lang="ts">
	import {
		portalCheck,
		portalGetStatus,
		portalGetExamSeat,
		portalConfirm,
		portalSubmitForm
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import {
		GraduationCap,
		Search,
		Check,
		FileText,
		AlertCircle,
		Clock,
		X,
		Edit3,
		ArrowLeft
	} from 'lucide-svelte';

	type PortalStep = 'login' | 'status';

	let step: PortalStep = $state('login');
	let checking = $state(false);
	let confirming = $state(false);
	let savingForm = $state(false);

	let nationalId = $state('');
	let dateOfBirth = $state(''); // format: DDMMYYYY พ.ศ.

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
		application?: {
			id: string;
			admissionRoundId: string;
			applicationNumber?: string;
			firstName: string;
			lastName: string;
			status: string;
			trackName?: string;
			roundName?: string;
			rejectionReason?: string;
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

	const statusLabel: Record<string, string> = {
		submitted: 'รอตรวจสอบเอกสาร',
		verified: 'เอกสารผ่านแล้ว รอวันสอบ',
		rejected: 'ไม่ผ่านการคัดเลือก',
		accepted: 'ผ่านการคัดเลือก!',
		enrolled: 'มอบตัวสำเร็จ',
		withdrawn: 'ถอนตัวแล้ว'
	};

	const statusColor: Record<string, string> = {
		submitted: 'bg-yellow-50 border-yellow-200 text-yellow-800',
		verified: 'bg-blue-50 border-blue-200 text-blue-800',
		rejected: 'bg-red-50 border-red-200 text-red-800',
		accepted: 'bg-green-50 border-green-200 text-green-800',
		enrolled: 'bg-purple-50 border-purple-200 text-purple-800',
		withdrawn: 'bg-gray-50 border-gray-200 text-gray-800'
	};

	// Enrollment form fields
	let formFields = $state({
		shirtSize: '',
		bloodType: '',
		allergy: '',
		congenitalDisease: '',
		emergencyContact: '',
		emergencyPhone: ''
	});

	async function handleCheck(e: Event) {
		e.preventDefault();
		const dob = dateOfBirth.trim();
		if (!nationalId.trim() || dob.length !== 8) {
			toast.error('กรุณากรอกข้อมูลให้ครบ (วันเกิด 8 หลัก)');
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
			if (portalData?.enrollmentForm?.formData) {
				const fd = portalData.enrollmentForm.formData as Record<string, string>;
				formFields.shirtSize = fd.shirtSize ?? '';
				formFields.bloodType = fd.bloodType ?? '';
				formFields.allergy = fd.allergy ?? '';
				formFields.congenitalDisease = fd.congenitalDisease ?? '';
				formFields.emergencyContact = fd.emergencyContact ?? '';
				formFields.emergencyPhone = fd.emergencyPhone ?? '';
			}

			// โหลดที่นั่งสอบถ้า round status อนุญาต
			const rs = portalData?.roundStatus;
			if (rs && ['exam_announced', 'announced', 'enrolling', 'closed'].includes(rs)) {
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

	async function handleConfirm() {
		confirming = true;
		try {
			await portalConfirm(nationalId, dateOfBirth.trim());
			toast.success('ยืนยันเข้าเรียนแล้ว กรุณากรอกแบบฟอร์มมอบตัว');
			await loadStatus();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยืนยันไม่สำเร็จ');
		} finally {
			confirming = false;
		}
	}

	async function handleSubmitForm(e: Event) {
		e.preventDefault();
		savingForm = true;
		try {
			await portalSubmitForm(nationalId, dateOfBirth.trim(), formFields as Record<string, unknown>);
			toast.success('บันทึกแบบฟอร์มสำเร็จ');
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
		<div class="flex items-center gap-2">
			<Button
				href="/apply"
				variant="ghost"
				size="sm"
				class="text-blue-600 hover:text-blue-700 hover:bg-blue-50"
			>
				<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
			</Button>
		</div>

		<!-- Header -->
		<div class="text-center">
			<div class="inline-flex p-3 bg-white rounded-2xl shadow-md mb-4">
				<GraduationCap class="w-10 h-10 text-blue-600" />
			</div>
			<h1 class="text-2xl font-bold text-gray-900">ตรวจสอบผลการสมัครเรียน</h1>
			<p class="text-gray-500 mt-1 text-sm">กรอกเลขบัตรประชาชนและวันเดือนปีเกิดเพื่อตรวจสอบผล</p>
		</div>

		{#if step === 'login'}
			<!-- Login Form -->
			<div class="bg-white rounded-2xl shadow-lg p-6">
				<form onsubmit={handleCheck} class="space-y-4">
					<div class="space-y-1.5">
						<label for="national-id" class="text-sm font-medium text-gray-700"
							>เลขบัตรประชาชน 13 หลัก</label
						>
						<Input
							id="national-id"
							bind:value={nationalId}
							maxlength={13}
							placeholder="X-XXXX-XXXXX-XX-X"
							class="h-11"
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
							placeholder="เช่น 20082543 (ดดมมปปปป 8 หลัก)"
							bind:value={dateOfBirth}
							class="h-11 font-mono tracking-widest text-center"
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

			<!-- Back -->
			<button
				onclick={() => {
					step = 'login';
					portalData = null;
				}}
				class="text-sm text-blue-600 hover:underline flex items-center gap-1"
			>
				← ตรวจสอบด้วยข้อมูลอื่น
			</button>

			<!-- Status Card -->
			{#if app}
				<div class="bg-white rounded-2xl shadow-lg p-6 space-y-4">
					<div class="flex items-start justify-between">
						<div>
							<p class="text-xs text-gray-400 uppercase tracking-wide">ผู้สมัคร</p>
							<p class="text-xl font-bold text-gray-900">{app.firstName} {app.lastName}</p>
							<p class="text-sm text-gray-500">
								ใบสมัคร: <span class="font-mono font-semibold">{app.applicationNumber}</span>
							</p>
							<p class="text-sm text-gray-500">
								สาย: {app.trackName ?? '-'} | รอบ: {app.roundName ?? '-'}
							</p>
						</div>
					</div>

					<div
						class="border rounded-xl p-4 {statusColor[app.status] ?? 'bg-gray-50 border-gray-200'}"
					>
						<div class="flex flex-col gap-2">
							<div class="flex items-center gap-2">
								{#if app.status === 'accepted' || app.status === 'enrolled'}
									<Check class="w-5 h-5" />
								{:else if app.status === 'rejected'}
									<X class="w-5 h-5" />
								{:else}
									<Clock class="w-5 h-5" />
								{/if}
								<p class="font-semibold">{statusLabel[app.status] ?? app.status}</p>
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
					{#if assignment && (app.status === 'accepted' || app.status === 'enrolled')}
						<div class="border border-green-200 bg-green-50 rounded-xl p-4 space-y-2">
							<p class="font-semibold text-green-800 flex items-center gap-2">
								<GraduationCap class="w-4 h-4" />
								ผลการคัดเลือก
							</p>
							<div class="grid grid-cols-3 gap-3 text-center">
								<div>
									<p class="text-2xl font-bold text-green-700">{assignment.rankInTrack ?? '-'}</p>
									<p class="text-xs text-green-600">อันดับในสาย</p>
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

						<!-- Confirm Button -->
						{#if app.status === 'accepted' && !assignment.studentConfirmed && roundStatus === 'enrolling'}
							<div class="border border-orange-200 bg-orange-50 rounded-xl p-4">
								<p class="text-sm font-medium text-orange-800 mb-3">
									<AlertCircle class="w-4 h-4 inline mr-1" />
									กรุณายืนยันเข้าเรียนภายในกำหนด
								</p>
								<Button
									onclick={handleConfirm}
									disabled={confirming}
									class="w-full gap-2 bg-orange-600 hover:bg-orange-700"
								>
									<Check class="w-4 h-4" />
									{confirming ? 'กำลังยืนยัน...' : 'ยืนยันเข้าเรียน'}
								</Button>
							</div>
						{:else if assignment.studentConfirmed}
							<div
								class="flex items-center gap-2 px-3 py-2 bg-green-100 text-green-700 rounded-lg text-sm"
							>
								<Check class="w-4 h-4" />
								ยืนยันเข้าเรียนแล้ว
							</div>
						{/if}
					{/if}

					<!-- Exam Seat -->
					{#if examSeat}
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
								<p class="text-xs text-blue-500 text-center font-mono">เลขประจำตัวสอบ: {examSeat.examId}</p>
							{/if}
						</div>
					{/if}

					<!-- Scores -->
					{#if scores && scores.length > 0 && scores.some((s) => s.score !== null)}
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

			<!-- Enrollment Form (if confirmed and in enrolling phase) -->
			{#if assignment?.studentConfirmed && app?.status !== 'enrolled' && roundStatus === 'enrolling'}
				<div class="bg-white rounded-2xl shadow-lg p-6">
					<h2 class="font-semibold text-gray-900 flex items-center gap-2 mb-4">
						<FileText class="w-5 h-5 text-blue-600" />
						แบบฟอร์มมอบตัว (กรอกล่วงหน้า)
					</h2>
					{#if form}
						<p class="text-xs text-green-600 mb-3">✓ บันทึกแล้ว — สามารถแก้ไขได้</p>
					{/if}
					<form onsubmit={handleSubmitForm} class="space-y-3">
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-1">
								<Label for="shirt-size" class="text-xs font-medium text-gray-600">ไซส์เสื้อ</Label>
								<Select.Root type="single" bind:value={formFields.shirtSize}>
									<Select.Trigger id="shirt-size" class="h-8 text-sm">
										{formFields.shirtSize || '-- เลือก --'}
									</Select.Trigger>
									<Select.Content>
										{#each ['XS', 'S', 'M', 'L', 'XL', 'XXL'] as s}
											<Select.Item value={s}>{s}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="space-y-1">
								<Label for="blood-type" class="text-xs font-medium text-gray-600">กลุ่มเลือด</Label>
								<Select.Root type="single" bind:value={formFields.bloodType}>
									<Select.Trigger id="blood-type" class="h-8 text-sm">
										{formFields.bloodType || '-- เลือก --'}
									</Select.Trigger>
									<Select.Content>
										{#each ['A', 'B', 'AB', 'O'] as b}
											<Select.Item value={b}>{b}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
						</div>
						<div class="space-y-1">
							<Label for="emergency-contact" class="text-xs font-medium text-gray-600"
								>ผู้ติดต่อฉุกเฉิน</Label
							>
							<Input
								id="emergency-contact"
								bind:value={formFields.emergencyContact}
								placeholder="ชื่อ-สกุล ผู้ติดต่อ"
								class="h-8 text-sm"
							/>
						</div>
						<div class="space-y-1">
							<Label for="emergency-phone" class="text-xs font-medium text-gray-600"
								>เบอร์โทรฉุกเฉิน</Label
							>
							<Input
								id="emergency-phone"
								bind:value={formFields.emergencyPhone}
								placeholder="0XX-XXX-XXXX"
								class="h-8 text-sm"
							/>
						</div>
						<div class="space-y-1">
							<Label for="allergy" class="text-xs font-medium text-gray-600"
								>โรคประจำตัว / แพ้ยา</Label
							>
							<Textarea
								id="allergy"
								bind:value={formFields.allergy}
								rows={2}
								class="text-sm resize-none"
								placeholder="ถ้าไม่มีใส่ - ไม่มี"
							/>
						</div>
						<Button
							type="submit"
							disabled={savingForm}
							class="w-full gap-2 text-white bg-blue-600 hover:bg-blue-700"
						>
							{savingForm ? 'กำลังบันทึก...' : form ? 'อัปเดตแบบฟอร์ม' : 'บันทึกแบบฟอร์ม'}
						</Button>
					</form>
				</div>
			{/if}
		{/if}
	</div>
</div>
