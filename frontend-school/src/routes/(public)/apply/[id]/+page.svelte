<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		submitApplication,
		getPublicRoundInfo,
		updateApplication,
		portalGetStatus
	} from '$lib/api/admission';
	import type { AdmissionRound, AdmissionTrack } from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import {
		GraduationCap,
		CheckCircle2,
		AlertCircle,
		ChevronRight,
		Loader2,
		Save
	} from 'lucide-svelte';

	let { data } = $props();

	let round = $state<AdmissionRound | null>(null);
	let tracks = $state<AdmissionTrack[]>([]);
	let loadError = $state('');
	let loading = $state(true);

	let submitting = $state(false);
	let successResult: { applicationNumber?: string; message?: string } | null = $state(null);

	let isEditMode = $state(false);
	let authNid = '';
	let authDob = '';

	onMount(async () => {
		const id = page.params.id;
		isEditMode = page.url.searchParams.get('edit') === 'true';

		if (isEditMode) {
			authNid = sessionStorage.getItem('admissionEditNid') || '';
			authDob = sessionStorage.getItem('admissionEditDob') || '';
		}

		if (!id) {
			loadError = 'ไม่พบ ID ของรอบ';
			loading = false;
			return;
		}

		try {
			const info = await getPublicRoundInfo(id);
			round = info.round;
			tracks = info.tracks;

			if (isEditMode && authNid && authDob) {
				const statusData = (await portalGetStatus(authNid, authDob)) as any;
				if (statusData?.application) {
					const app = statusData.application;
					trackId = app.admissionTrackId || '';
					nationalId = app.nationalId || '';
					title = app.title || '';
					firstName = app.firstName || '';
					lastName = app.lastName || '';
					gender = app.gender || '';
					if (app.dateOfBirth) {
						if (app.dateOfBirth.length === 10) {
							// dateOfBirth is returned as YYYY-MM-DD from the backend (NaiveDate)
							dob = app.dateOfBirth;
						}
					}
					phone = app.phone || '';
					email = app.email || '';
					addressLine = app.addressLine || '';
					subDistrict = app.subDistrict || '';
					district = app.district || '';
					province = app.province || '';
					postalCode = app.postalCode || '';
					previousSchool = app.previousSchool || '';
					previousGrade = app.previousGrade || '';
					previousGpa = app.previousGpa ? app.previousGpa.toString() : '';

					const parseName = (
						fullName: string,
						setTitle: (v: string) => void,
						setFirst: (v: string) => void,
						setLast: (v: string) => void
					) => {
						if (!fullName) return;
						let f = fullName.trim();
						if (f.startsWith('นาย')) {
							setTitle('นาย');
							f = f.substring(3).trim();
						} else if (f.startsWith('นางสาว')) {
							setTitle('นางสาว');
							f = f.substring(6).trim();
						} else if (f.startsWith('นาง')) {
							setTitle('นาง');
							f = f.substring(3).trim();
						} else if (f.startsWith('ด.ช.')) {
							setTitle('ด.ช.');
							f = f.substring(4).trim();
						} else if (f.startsWith('ด.ญ.')) {
							setTitle('ด.ญ.');
							f = f.substring(4).trim();
						} else {
							setTitle('');
						}
						const parts = f.split(' ').filter(Boolean);
						setFirst(parts[0] || '');
						setLast(parts.slice(1).join(' '));
					};

					parseName(
						app.fatherName || '',
						(v: string) => (fatherTitle = v),
						(v: string) => (fatherFirstName = v),
						(v: string) => (fatherLastName = v)
					);
					parseName(
						app.motherName || '',
						(v: string) => (motherTitle = v),
						(v: string) => (motherFirstName = v),
						(v: string) => (motherLastName = v)
					);
					parseName(
						app.guardianName || '',
						(v: string) => (guardianTitle = v),
						(v: string) => (guardianFirstName = v),
						(v: string) => (guardianLastName = v)
					);

					fatherPhone = app.fatherPhone || '';
					fatherOccupation = app.fatherOccupation || '';
					fatherNationalId = app.fatherNationalId || '';

					motherPhone = app.motherPhone || '';
					motherOccupation = app.motherOccupation || '';
					motherNationalId = app.motherNationalId || '';

					guardianRelation = app.guardianRelation || '';
					guardianPhone = app.guardianPhone || '';
					guardianNationalId = app.guardianNationalId || '';
				}
			}
		} catch (e) {
			loadError = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลรอบรับสมัคร';
		} finally {
			loading = false;
		}
	});

	let trackId = $state('');
	let nationalId = $state('');
	let title = $state('');
	let firstName = $state('');
	let lastName = $state('');
	let gender = $state('');
	let dob = $state('');
	let phone = $state('');
	let email = $state('');
	let addressLine = $state('');
	let subDistrict = $state('');
	let district = $state('');
	let province = $state('');
	let postalCode = $state('');
	let previousSchool = $state('');
	let previousGrade = $state('');
	let previousGpa = $state('');

	let fatherTitle = $state('');
	let fatherFirstName = $state('');
	let fatherLastName = $state('');
	let fatherPhone = $state('');
	let fatherOccupation = $state('');
	let fatherNationalId = $state('');

	let motherTitle = $state('');
	let motherFirstName = $state('');
	let motherLastName = $state('');
	let motherPhone = $state('');
	let motherOccupation = $state('');
	let motherNationalId = $state('');

	let guardianTitle = $state('');
	let guardianFirstName = $state('');
	let guardianLastName = $state('');
	let guardianPhone = $state('');
	let guardianRelation = $state('');
	let guardianNationalId = $state('');

	async function handleSubmit(e: Event) {
		e.preventDefault();

		if (!trackId) {
			toast.error('กรุณาเลือกสายการเรียน');
			return;
		}
		if (nationalId.length !== 13) {
			toast.error('กรุณากรอกเลขบัตรประชาชน 13 หลัก');
			return;
		}

		submitting = true;
		try {
			let res;
			if (isEditMode) {
				const payload = {
					admissionTrackId: trackId,
					nationalId,
					title,
					firstName,
					lastName,
					gender,
					dateOfBirth: dob || undefined,
					phone,
					email,
					addressLine,
					subDistrict,
					district,
					province,
					postalCode,
					previousSchool,
					previousGrade,
					previousGpa: previousGpa ? parseFloat(previousGpa) : undefined,
					fatherName: fatherFirstName
						? `${fatherTitle}${fatherFirstName} ${fatherLastName}`.trim()
						: undefined,
					fatherPhone,
					fatherOccupation,
					fatherNationalId,
					motherName: motherFirstName
						? `${motherTitle}${motherFirstName} ${motherLastName}`.trim()
						: undefined,
					motherPhone,
					motherOccupation,
					motherNationalId,
					guardianName: guardianFirstName
						? `${guardianTitle}${guardianFirstName} ${guardianLastName}`.trim()
						: undefined,
					guardianPhone,
					guardianRelation,
					guardianNationalId
				};
				await updateApplication(authNid, authDob, payload);
				res = { message: 'อัปเดตใบสมัครเรียบร้อยแล้ว' };
			} else {
				res = await submitApplication(page.params.id!, {
					admissionTrackId: trackId,
					nationalId,
					title,
					firstName,
					lastName,
					gender,
					dateOfBirth: dob || undefined,
					phone,
					email,
					addressLine,
					subDistrict,
					district,
					province,
					postalCode,
					previousSchool,
					previousGrade,
					previousGpa: previousGpa ? parseFloat(previousGpa) : undefined,
					fatherName: fatherFirstName
						? `${fatherTitle}${fatherFirstName} ${fatherLastName}`.trim()
						: undefined,
					fatherPhone,
					fatherOccupation,
					fatherNationalId,
					motherName: motherFirstName
						? `${motherTitle}${motherFirstName} ${motherLastName}`.trim()
						: undefined,
					motherPhone,
					motherOccupation,
					motherNationalId,
					guardianName: guardianFirstName
						? `${guardianTitle}${guardianFirstName} ${guardianLastName}`.trim()
						: undefined,
					guardianPhone,
					guardianRelation,
					guardianNationalId
				});
			}

			successResult = res;
			window.scrollTo({ top: 0, behavior: 'smooth' });
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เกิดข้อผิดพลาด กรุณาลองใหม่');
		} finally {
			submitting = false;
		}
	}
</script>

<svelte:head>
	<title>{round ? `สมัครเรียน – ${round.name}` : 'สมัครเรียน'} | SchoolOrbit</title>
</svelte:head>

<div class="min-h-screen bg-gradient-to-b from-blue-50 to-white py-10 px-4">
	<div class="max-w-3xl mx-auto space-y-6">
		<!-- Loading -->
		{#if loading}
			<div class="flex flex-col items-center justify-center py-24 gap-4 text-muted-foreground">
				<Loader2 class="w-10 h-10 animate-spin text-blue-500" />
				<p>กำลังโหลดข้อมูลรอบรับสมัคร...</p>
			</div>

			<!-- Error -->
		{:else if loadError}
			<div class="flex flex-col items-center justify-center py-24 gap-4 text-center">
				<AlertCircle class="w-12 h-12 text-red-400" />
				<p class="text-gray-700 font-medium">{loadError}</p>
				<Button onclick={() => window.location.reload()} variant="outline">ลองใหม่</Button>
			</div>
		{:else}
			<!-- Header -->
			<div class="text-center space-y-2 pb-2">
				<div class="inline-flex p-3 bg-white rounded-2xl shadow border border-blue-100 mb-2">
					<GraduationCap class="w-10 h-10 text-blue-600" />
				</div>
				<h1 class="text-3xl font-bold text-gray-900">
					{isEditMode ? 'แก้ไขใบสมัครเรียน' : 'ใบสมัครเรียน'}
				</h1>
				{#if round}
					<p class="text-lg font-medium text-blue-700">{round.name}</p>
					<p class="text-sm text-gray-500">
						ระดับชั้น {round.gradeLevelName} | ปีการศึกษา {round.academicYearName}
					</p>
					<p class="text-xs text-gray-400 mt-1">
						รับสมัคร {new Date(round.applyStartDate).toLocaleDateString('th-TH', {
							year: 'numeric',
							month: 'short',
							day: 'numeric'
						})}
						– {new Date(round.applyEndDate).toLocaleDateString('th-TH', {
							year: 'numeric',
							month: 'short',
							day: 'numeric'
						})}
					</p>
				{/if}
			</div>

			<!-- Success -->
			{#if successResult}
				<Card.Root class="border-green-200 shadow-lg">
					<Card.Content class="pt-8 pb-8 text-center space-y-5">
						<CheckCircle2 class="w-20 h-20 text-green-500 mx-auto" />
						<div>
							<h2 class="text-2xl font-bold text-green-800 mb-1">
								{isEditMode ? 'อัปเดตใบสมัครสำเร็จ!' : 'ส่งใบสมัครสำเร็จ!'}
							</h2>
							<p class="text-gray-600">ได้รับข้อมูลใบสมัครของท่านเรียบร้อยแล้ว</p>
						</div>
						<div
							class="flex items-start gap-3 max-w-sm mx-auto bg-amber-50 border border-amber-200 text-amber-800 rounded-lg p-4 text-sm text-left"
						>
							<AlertCircle class="w-5 h-5 shrink-0 mt-0.5" />
							{#if !isEditMode}
								<p>
									ระบบใช้ <strong>เลขบัตรประชาชน</strong> และ
									<strong>วัน/เดือน/ปีเกิด (พ.ศ.)</strong>
									ของผู้สมัครในการตรวจสอบสถานะในภายหลัง
								</p>
							{:else}
								<p>
									ข้อมูลใบสมัครของคุณได้รับการแก้ไขและอัปเดตเรียบร้อยแล้ว <br /> กรุณารอการตรวจสอบจากฝั่งเจ้าหน้าที่อีกครั้ง
								</p>
							{/if}
						</div>
						<Button href="/apply" class="gap-2 mt-2">
							<ChevronRight class="w-4 h-4" /> ไปหน้าตรวจสอบผลการสมัคร
						</Button>
					</Card.Content>
				</Card.Root>

				<!-- Form -->
			{:else}
				<form onsubmit={handleSubmit} novalidate>
					<!-- Step 1: เลือกสายการเรียน -->
					<Card.Root class="mb-5 shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">
								1. เลือกสายการเรียน <span class="text-red-500">*</span>
							</Card.Title>
						</Card.Header>
						<Card.Content>
							{#if tracks.length === 0}
								<p class="text-sm text-muted-foreground py-4 text-center">
									ไม่มีสายการเรียนที่เปิดรับในรอบนี้
								</p>
							{:else}
								<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
									{#each tracks as t}
										<button
											type="button"
											onclick={() => (trackId = t.id)}
											class="text-left border-2 rounded-xl p-4 transition-all
											{trackId === t.id
												? 'border-primary bg-primary/5'
												: 'border-border hover:border-primary/40 bg-card'}"
										>
											<div class="flex items-center gap-2">
												<div
													class="w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0
													{trackId === t.id ? 'border-primary' : 'border-muted-foreground'}"
												>
													{#if trackId === t.id}
														<div class="w-2 h-2 rounded-full bg-primary"></div>
													{/if}
												</div>
												<p class="font-semibold text-card-foreground">{t.name}</p>
											</div>
											{#if t.studyPlanName}
												<p class="text-xs text-muted-foreground mt-1 ml-6">{t.studyPlanName}</p>
											{/if}
										</button>
									{/each}
								</div>
							{/if}
						</Card.Content>
					</Card.Root>

					<!-- Step 2: ข้อมูลผู้สมัคร -->
					<Card.Root class="mb-5 shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">2. ข้อมูลผู้สมัคร</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-5">
							<div class="space-y-2">
								<Label for="nationalId"
									>เลขประจำตัวประชาชน <span class="text-red-500">*</span></Label
								>
								<Input
									id="nationalId"
									bind:value={nationalId}
									maxlength={13}
									required
									placeholder="X-XXXX-XXXXX-XX-X"
									class="font-mono text-lg max-w-sm"
								/>
							</div>

							<div class="grid grid-cols-12 gap-3">
								<div class="col-span-12 sm:col-span-3 space-y-2">
									<Label for="title-select">คำนำหน้า <span class="text-red-500">*</span></Label>
									<Select.Root type="single" bind:value={title} required>
										<Select.Trigger id="title-select">
											{title || '-- เลือก --'}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="ด.ช.">เด็กชาย (ด.ช.)</Select.Item>
											<Select.Item value="ด.ญ.">เด็กหญิง (ด.ญ.)</Select.Item>
											<Select.Item value="นาย">นาย</Select.Item>
											<Select.Item value="นางสาว">นางสาว</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div class="col-span-12 sm:col-span-5 space-y-2">
									<Label for="firstName">ชื่อจริง <span class="text-red-500">*</span></Label>
									<Input id="firstName" bind:value={firstName} required />
								</div>
								<div class="col-span-12 sm:col-span-4 space-y-2">
									<Label for="lastName">นามสกุล <span class="text-red-500">*</span></Label>
									<Input id="lastName" bind:value={lastName} required />
								</div>
							</div>

							<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
								<div class="space-y-2">
									<Label for="gender-select">เพศ</Label>
									<Select.Root type="single" bind:value={gender}>
										<Select.Trigger id="gender-select">
											{gender === 'Male' ? 'ชาย' : gender === 'Female' ? 'หญิง' : '-- เลือก --'}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="Male">ชาย</Select.Item>
											<Select.Item value="Female">หญิง</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-2">
									<Label for="dob">วันเกิด (ปฏิทินไทย)</Label>
									<DatePicker bind:value={dob} />
								</div>
								<div class="space-y-2">
									<Label for="phone">เบอร์โทร</Label>
									<Input id="phone" type="tel" bind:value={phone} placeholder="08XXXXXXXX" />
								</div>
							</div>

							<div class="space-y-2">
								<Label for="email">อีเมล</Label>
								<Input id="email" type="email" bind:value={email} placeholder="example@email.com" />
							</div>

							<Separator />

							<p class="text-sm font-semibold">ที่อยู่ปัจจุบัน</p>
							<div class="space-y-2">
								<Label for="addressLine">บ้านเลขที่ / ซอย / ถนน</Label>
								<Input id="addressLine" bind:value={addressLine} />
							</div>
							<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<div class="col-span-2 space-y-2">
									<Label for="subDistrict">แขวง / ตำบล</Label>
									<Input id="subDistrict" bind:value={subDistrict} />
								</div>
								<div class="col-span-1 space-y-2">
									<Label for="district">เขต / อำเภอ</Label>
									<Input id="district" bind:value={district} />
								</div>
								<div class="col-span-1 space-y-2">
									<Label for="province">จังหวัด</Label>
									<Input id="province" bind:value={province} />
								</div>
								<div class="col-span-1 space-y-2">
									<Label for="postalCode">รหัสไปรษณีย์</Label>
									<Input id="postalCode" bind:value={postalCode} maxlength={5} />
								</div>
							</div>

							<Separator />

							<p class="text-sm font-semibold">ข้อมูลโรงเรียนเดิม</p>
							<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
								<div class="col-span-2 space-y-2">
									<Label for="prevSchool">ชื่อโรงเรียนเดิม</Label>
									<Input id="prevSchool" bind:value={previousSchool} />
								</div>
								<div class="space-y-2">
									<Label for="prevGrade">ชั้นที่จบ</Label>
									<Select.Root type="single" bind:value={previousGrade}>
										<Select.Trigger id="prevGrade" class="w-full">
											{previousGrade || '-- เลือกระดับชั้น --'}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="อนุบาล 3">อนุบาล 3</Select.Item>
											<Select.Item value="ประถมศึกษาปีที่ 6">ประถมศึกษาปีที่ 6</Select.Item>
											<Select.Item value="มัธยมศึกษาปีที่ 3">มัธยมศึกษาปีที่ 3</Select.Item>
											<Select.Item value="เทียบเท่า">เทียบเท่า / อื่นๆ</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
								<div class="space-y-2">
									<Label for="prevGpa">เกรดเฉลี่ย (GPA)</Label>
									<Input
										id="prevGpa"
										type="number"
										bind:value={previousGpa}
										step="0.01"
										min="0"
										max="4"
										placeholder="0.00 – 4.00"
									/>
								</div>
							</div>
						</Card.Content>
					</Card.Root>

					<!-- Step 3: ข้อมูลครอบครัว -->
					<Card.Root class="mb-5 shadow-sm">
						<Card.Header class="pb-2">
							<Card.Title class="text-base">3. ข้อมูลครอบครัว</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-6">
							<!-- บิดา -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
									บิดา
								</p>
								<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
									<div class="grid grid-cols-12 gap-3 sm:col-span-2">
										<div class="col-span-12 sm:col-span-3 space-y-2">
											<Label for="fatherTitle">คำนำหน้า</Label>
											<Select.Root type="single" bind:value={fatherTitle}>
												<Select.Trigger id="fatherTitle"
													>{fatherTitle || '-- เลือก --'}</Select.Trigger
												>
												<Select.Content>
													<Select.Item value="นาย">นาย</Select.Item>
													<Select.Item value="ม.ร.ว.">ม.ร.ว.</Select.Item>
													<Select.Item value="ม.ล.">ม.ล.</Select.Item>
													<Select.Item value="ดร.">ดร.</Select.Item>
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-span-12 sm:col-span-5 space-y-2">
											<Label for="fatherFirstName">ชื่อ</Label>
											<Input
												id="fatherFirstName"
												bind:value={fatherFirstName}
												placeholder="สมชาย"
											/>
										</div>
										<div class="col-span-12 sm:col-span-4 space-y-2">
											<Label for="fatherLastName">นามสกุล</Label>
											<Input id="fatherLastName" bind:value={fatherLastName} placeholder="ใจดี" />
										</div>
									</div>
									<div class="space-y-2">
										<Label for="fatherNationalId">เลขประชาชน</Label>
										<Input
											id="fatherNationalId"
											bind:value={fatherNationalId}
											maxlength={13}
											class="font-mono"
										/>
									</div>
									<div class="space-y-2">
										<Label for="fatherPhone">เบอร์โทร</Label>
										<Input id="fatherPhone" bind:value={fatherPhone} type="tel" />
									</div>
									<div class="space-y-2">
										<Label for="fatherOcc">อาชีพ</Label>
										<Input id="fatherOcc" bind:value={fatherOccupation} />
									</div>
								</div>
							</div>

							<Separator />

							<!-- มารดา -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
									มารดา
								</p>
								<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
									<div class="grid grid-cols-12 gap-3 sm:col-span-2">
										<div class="col-span-12 sm:col-span-3 space-y-2">
											<Label for="motherTitle">คำนำหน้า</Label>
											<Select.Root type="single" bind:value={motherTitle}>
												<Select.Trigger id="motherTitle"
													>{motherTitle || '-- เลือก --'}</Select.Trigger
												>
												<Select.Content>
													<Select.Item value="นาง">นาง</Select.Item>
													<Select.Item value="นางสาว">นางสาว</Select.Item>
													<Select.Item value="ม.ร.ว.">ม.ร.ว.</Select.Item>
													<Select.Item value="ม.ล.">ม.ล.</Select.Item>
													<Select.Item value="ดร.">ดร.</Select.Item>
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-span-12 sm:col-span-5 space-y-2">
											<Label for="motherFirstName">ชื่อ</Label>
											<Input
												id="motherFirstName"
												bind:value={motherFirstName}
												placeholder="สมศรี"
											/>
										</div>
										<div class="col-span-12 sm:col-span-4 space-y-2">
											<Label for="motherLastName">นามสกุล</Label>
											<Input id="motherLastName" bind:value={motherLastName} placeholder="ใจดี" />
										</div>
									</div>
									<div class="space-y-2">
										<Label for="motherNationalId">เลขประชาชน</Label>
										<Input
											id="motherNationalId"
											bind:value={motherNationalId}
											maxlength={13}
											class="font-mono"
										/>
									</div>
									<div class="space-y-2">
										<Label for="motherPhone">เบอร์โทร</Label>
										<Input id="motherPhone" bind:value={motherPhone} type="tel" />
									</div>
									<div class="space-y-2">
										<Label for="motherOcc">อาชีพ</Label>
										<Input id="motherOcc" bind:value={motherOccupation} />
									</div>
								</div>
							</div>

							<Separator />

							<!-- ผู้ปกครอง -->
							<div class="space-y-3">
								<p class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
									ผู้ปกครอง
									<span class="normal-case font-normal text-muted-foreground/70">
										(กรณีไม่ใช่บิดา-มารดา)
									</span>
								</p>
								<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
									<div class="grid grid-cols-12 gap-3 sm:col-span-2">
										<div class="col-span-12 sm:col-span-3 space-y-2">
											<Label for="guardianTitle">คำนำหน้า <span class="text-red-500">*</span></Label
											>
											<Select.Root type="single" bind:value={guardianTitle} required>
												<Select.Trigger id="guardianTitle"
													>{guardianTitle || '-- เลือก --'}</Select.Trigger
												>
												<Select.Content>
													<Select.Item value="นาย">นาย</Select.Item>
													<Select.Item value="นาง">นาง</Select.Item>
													<Select.Item value="นางสาว">นางสาว</Select.Item>
													<Select.Item value="ปู่">ปู่</Select.Item>
													<Select.Item value="ย่า">ย่า</Select.Item>
													<Select.Item value="ตา">ตา</Select.Item>
													<Select.Item value="ยาย">ยาย</Select.Item>
													<Select.Item value="ดร.">ดร.</Select.Item>
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-span-12 sm:col-span-5 space-y-2">
											<Label for="guardianFirstName">ชื่อ <span class="text-red-500">*</span></Label
											>
											<Input
												id="guardianFirstName"
												bind:value={guardianFirstName}
												placeholder="สมศักดิ์"
												required
											/>
										</div>
										<div class="col-span-12 sm:col-span-4 space-y-2">
											<Label for="guardianLastName"
												>นามสกุล <span class="text-red-500">*</span></Label
											>
											<Input
												id="guardianLastName"
												bind:value={guardianLastName}
												placeholder="ใจดี"
												required
											/>
										</div>
									</div>
									<div class="space-y-2">
										<Label for="guardianRelation">ความสัมพันธ์</Label>
										<Input
											id="guardianRelation"
											bind:value={guardianRelation}
											placeholder="เช่น ปู่, ย่า, ลุง"
										/>
									</div>
									<div class="space-y-2">
										<Label for="guardianPhone">เบอร์โทร</Label>
										<Input id="guardianPhone" bind:value={guardianPhone} type="tel" />
									</div>
									<div class="space-y-2">
										<Label for="guardianNationalId">เลขประชาชน</Label>
										<Input
											id="guardianNationalId"
											bind:value={guardianNationalId}
											maxlength={13}
											class="font-mono"
										/>
									</div>
								</div>
							</div>
						</Card.Content>
					</Card.Root>

					<!-- Submit -->
					<div class="flex flex-col sm:flex-row items-center justify-between gap-4 pt-2 pb-8">
						<p class="text-xs text-muted-foreground text-center sm:text-left max-w-sm">
							ข้าพเจ้าขอรับรองว่าข้อมูลที่กรอกทั้งหมดเป็นความจริง
						</p>
						<Button type="submit" size="lg" disabled={submitting} class="w-full sm:w-auto px-10">
							{submitting
								? isEditMode
									? '⏳ กำลังบันทึก...'
									: '⏳ กำลังส่งข้อมูล...'
								: isEditMode
									? '💾 บันทึกการแก้ไข'
									: '📨 ส่งใบสมัคร'}
						</Button>
					</div>
				</form>
			{/if}
		{/if}<!-- /loading block -->
	</div>
</div>
