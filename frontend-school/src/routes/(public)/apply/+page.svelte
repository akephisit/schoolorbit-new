<script lang="ts">
	import { onMount } from 'svelte';
	import {
		portalCheck,
		portalGetStatus,
		portalConfirm,
		portalSubmitForm
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import { GraduationCap, Search, Check, FileText, AlertCircle, Clock, X } from 'lucide-svelte';

	type PortalStep = 'login' | 'status';

	let step: PortalStep = $state('login');
	let checking = $state(false);
	let confirming = $state(false);
	let savingForm = $state(false);

	let nationalId = $state('');
	let applicationNumber = $state('');

	let portalData: {
		application?: {
			id: string;
			applicationNumber?: string;
			firstName: string;
			lastName: string;
			status: string;
			trackName?: string;
			roundName?: string;
		};
		assignment?: {
			rankInTrack?: number;
			rankInRoom?: number;
			totalScore?: number;
			roomName?: string;
			studentConfirmed: boolean;
		};
		scores?: { subjectName?: string; score?: number; maxScore?: number }[];
		enrollmentForm?: { formData: Record<string, unknown>; preSubmittedAt?: string };
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

	// Enrollment form fields (basic — can extend)
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
		if (!nationalId.trim() || !applicationNumber.trim()) {
			toast.error('กรุณากรอกข้อมูลให้ครบ');
			return;
		}
		checking = true;
		try {
			await portalCheck(nationalId, applicationNumber);
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
			portalData = (await portalGetStatus(nationalId, applicationNumber)) as typeof portalData;
			// Pre-fill form if exists
			if (portalData?.enrollmentForm?.formData) {
				const fd = portalData.enrollmentForm.formData as Record<string, string>;
				formFields.shirtSize = fd.shirtSize ?? '';
				formFields.bloodType = fd.bloodType ?? '';
				formFields.allergy = fd.allergy ?? '';
				formFields.congenitalDisease = fd.congenitalDisease ?? '';
				formFields.emergencyContact = fd.emergencyContact ?? '';
				formFields.emergencyPhone = fd.emergencyPhone ?? '';
			}
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		}
	}

	async function handleConfirm() {
		confirming = true;
		try {
			await portalConfirm(nationalId, applicationNumber);
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
			await portalSubmitForm(nationalId, applicationNumber, formFields as Record<string, unknown>);
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
		<!-- Header -->
		<div class="text-center">
			<div class="inline-flex p-3 bg-white rounded-2xl shadow-md mb-4">
				<GraduationCap class="w-10 h-10 text-blue-600" />
			</div>
			<h1 class="text-2xl font-bold text-gray-900">ตรวจสอบผลการสมัครเรียน</h1>
			<p class="text-gray-500 mt-1 text-sm">กรอกเลขบัตรประชาชนและเลขที่ใบสมัครเพื่อตรวจสอบผล</p>
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
						<label for="app-number" class="text-sm font-medium text-gray-700">เลขที่ใบสมัคร</label>
						<Input
							id="app-number"
							bind:value={applicationNumber}
							placeholder="เช่น 2569-0001"
							class="h-11"
						/>
					</div>
					<Button type="submit" disabled={checking} class="w-full h-11 gap-2">
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

					<!-- Status Badge -->
					<div
						class="border rounded-xl p-4 {statusColor[app.status] ?? 'bg-gray-50 border-gray-200'}"
					>
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
						{#if app.status === 'accepted' && !assignment.studentConfirmed}
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

			<!-- Enrollment Form (if confirmed) -->
			{#if assignment?.studentConfirmed && app?.status !== 'enrolled'}
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
								<label for="shirt-size" class="text-xs font-medium text-gray-600">ไซส์เสื้อ</label>
								<select
									id="shirt-size"
									bind:value={formFields.shirtSize}
									class="w-full px-3 py-1.5 text-sm rounded-md border border-gray-300"
								>
									<option value="">-- เลือก --</option>
									{#each ['XS', 'S', 'M', 'L', 'XL', 'XXL'] as s}
										<option value={s}>{s}</option>
									{/each}
								</select>
							</div>
							<div class="space-y-1">
								<label for="blood-type" class="text-xs font-medium text-gray-600">กลุ่มเลือด</label>
								<select
									id="blood-type"
									bind:value={formFields.bloodType}
									class="w-full px-3 py-1.5 text-sm rounded-md border border-gray-300"
								>
									<option value="">-- เลือก --</option>
									{#each ['A', 'B', 'AB', 'O'] as b}
										<option value={b}>{b}</option>
									{/each}
								</select>
							</div>
						</div>
						<div class="space-y-1">
							<label for="emergency-contact" class="text-xs font-medium text-gray-600"
								>ผู้ติดต่อฉุกเฉิน</label
							>
							<Input
								id="emergency-contact"
								bind:value={formFields.emergencyContact}
								placeholder="ชื่อ-สกุล ผู้ติดต่อ"
								class="h-8 text-sm"
							/>
						</div>
						<div class="space-y-1">
							<label for="emergency-phone" class="text-xs font-medium text-gray-600"
								>เบอร์โทรฉุกเฉิน</label
							>
							<Input
								id="emergency-phone"
								bind:value={formFields.emergencyPhone}
								placeholder="0XX-XXX-XXXX"
								class="h-8 text-sm"
							/>
						</div>
						<div class="space-y-1">
							<label for="allergy" class="text-xs font-medium text-gray-600"
								>โรคประจำตัว / แพ้ยา</label
							>
							<textarea
								id="allergy"
								bind:value={formFields.allergy}
								rows="2"
								class="w-full px-3 py-1.5 text-sm rounded-md border border-gray-300 resize-none"
								placeholder="ถ้าไม่มีใส่ - ไม่มี"
							></textarea>
						</div>
						<Button type="submit" disabled={savingForm} class="w-full gap-2">
							{savingForm ? 'กำลังบันทึก...' : form ? 'อัปเดตแบบฟอร์ม' : 'บันทึกแบบฟอร์ม'}
						</Button>
					</form>
				</div>
			{/if}
		{/if}
	</div>
</div>
