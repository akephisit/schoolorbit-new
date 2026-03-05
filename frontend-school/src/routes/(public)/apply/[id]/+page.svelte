<script lang="ts">
	import { page } from '$app/stores';
	import { submitApplication } from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import { Separator } from '$lib/components/ui/separator';
	import { GraduationCap, CheckCircle2, AlertCircle, ChevronRight } from 'lucide-svelte';

	let { data } = $props();
	$inspect(data);

	const round = $derived(data.info?.round);
	const tracks = $derived(data.info?.tracks ?? []);

	let submitting = $state(false);
	let successResult: { applicationNumber: string } | null = $state(null);

	// Form state
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
	let fatherName = $state('');
	let fatherPhone = $state('');
	let fatherOccupation = $state('');
	let fatherNationalId = $state('');
	let motherName = $state('');
	let motherPhone = $state('');
	let motherOccupation = $state('');
	let motherNationalId = $state('');
	let guardianName = $state('');
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
			const res = await submitApplication($page.params.id, {
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
				fatherName,
				fatherPhone,
				fatherOccupation,
				fatherNationalId,
				motherName,
				motherPhone,
				motherOccupation,
				motherNationalId,
				guardianName,
				guardianPhone,
				guardianRelation,
				guardianNationalId
			});
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
		<!-- Header -->
		<div class="text-center space-y-2 pb-2">
			<div class="inline-flex p-3 bg-white rounded-2xl shadow border border-blue-100 mb-2">
				<GraduationCap class="w-10 h-10 text-blue-600" />
			</div>
			<h1 class="text-3xl font-bold text-gray-900">ใบสมัครเรียน</h1>
			{#if round}
				<p class="text-lg font-medium text-blue-700">{round.name}</p>
				<p class="text-sm text-gray-500">
					ระดับชั้น {round.gradeLevelName} | ปีการศึกษา {round.academicYearName}
				</p>
				<p class="text-xs text-gray-400 mt-1">
					เปิดรับสมัคร {new Date(round.applyStartDate).toLocaleDateString('th-TH', {
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

		<!-- Success State -->
		{#if successResult}
			<Card.Root class="border-green-200 shadow-lg">
				<Card.Content class="pt-8 pb-8 text-center space-y-5">
					<CheckCircle2 class="w-20 h-20 text-green-500 mx-auto" />
					<div>
						<h2 class="text-2xl font-bold text-green-800 mb-1">ส่งใบสมัครสำเร็จ!</h2>
						<p class="text-gray-600">กรุณาจดเลขที่ใบสมัครไว้ใช้ติดตามผล</p>
					</div>
					<div
						class="bg-green-50 border border-green-200 rounded-xl py-5 px-8 inline-block mx-auto"
					>
						<p class="text-sm text-gray-500 mb-1">เลขที่ใบสมัคร</p>
						<p class="text-4xl font-mono font-bold text-gray-900 tracking-widest">
							{successResult.applicationNumber}
						</p>
					</div>
					<div
						class="flex items-start gap-3 max-w-sm mx-auto bg-amber-50 border border-amber-200 text-amber-800 rounded-lg p-4 text-sm text-left"
					>
						<AlertCircle class="w-5 h-5 shrink-0 mt-0.5" />
						<p>
							โปรดจดจำ <strong>เลขที่ใบสมัคร</strong> และ <strong>เลขบัตรประชาชน</strong> เพื่อตรวจสอบสถานะในภายหลัง
						</p>
					</div>
					<Button href="/apply" variant="default" class="gap-2 mt-2">
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
						<Card.Title class="text-base"
							>1. เลือกสายการเรียน <span class="text-red-500">*</span></Card.Title
						>
					</Card.Header>
					<Card.Content>
						{#if tracks.length === 0}
							<p class="text-sm text-gray-500 py-4 text-center">
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
											? 'border-blue-500 bg-blue-50'
											: 'border-gray-200 hover:border-blue-300 bg-white'}"
									>
										<div class="flex items-center gap-2">
											<div
												class="w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0
												{trackId === t.id ? 'border-blue-500' : 'border-gray-400'}"
											>
												{#if trackId === t.id}
													<div class="w-2 h-2 rounded-full bg-blue-500"></div>
												{/if}
											</div>
											<p class="font-semibold text-gray-900">{t.name}</p>
										</div>
										{#if t.studyPlanName}
											<p class="text-xs text-gray-500 mt-1 ml-6">{t.studyPlanName}</p>
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
							<Label for="nationalId">เลขประจำตัวประชาชน <span class="text-red-500">*</span></Label>
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
								<Label for="title">คำนำหน้า <span class="text-red-500">*</span></Label>
								<select
									id="title"
									bind:value={title}
									required
									class="w-full h-10 border border-input rounded-md px-3 text-sm bg-background focus:outline-none focus:ring-2 focus:ring-ring"
								>
									<option value="">-- เลือก --</option>
									<option value="ด.ช.">เด็กชาย (ด.ช.)</option>
									<option value="ด.ญ.">เด็กหญิง (ด.ญ.)</option>
									<option value="นาย">นาย</option>
									<option value="นางสาว">นางสาว</option>
								</select>
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
								<Label for="gender">เพศ</Label>
								<select
									id="gender"
									bind:value={gender}
									class="w-full h-10 border border-input rounded-md px-3 text-sm bg-background focus:outline-none focus:ring-2 focus:ring-ring"
								>
									<option value="">-- เลือก --</option>
									<option value="Male">ชาย</option>
									<option value="Female">หญิง</option>
								</select>
							</div>
							<div class="space-y-2">
								<Label for="dob">วันเกิด</Label>
								<Input id="dob" type="date" bind:value={dob} />
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

						<p class="text-sm font-semibold text-gray-700">ที่อยู่ปัจจุบัน</p>
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

						<p class="text-sm font-semibold text-gray-700">ข้อมูลโรงเรียนเดิม</p>
						<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
							<div class="col-span-2 space-y-2">
								<Label for="prevSchool">ชื่อโรงเรียนเดิม</Label>
								<Input id="prevSchool" bind:value={previousSchool} />
							</div>
							<div class="space-y-2">
								<Label for="prevGrade">ชั้นที่จบ</Label>
								<Input id="prevGrade" bind:value={previousGrade} placeholder="เช่น ป.6" />
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
							<p class="text-xs font-semibold uppercase tracking-wide text-gray-500">บิดา</p>
							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
								<div class="space-y-2">
									<Label>ชื่อ-นามสกุล</Label>
									<Input bind:value={fatherName} placeholder="นายสมชาย ใจดี" />
								</div>
								<div class="space-y-2">
									<Label>เลขประชาชน</Label>
									<Input bind:value={fatherNationalId} maxlength={13} class="font-mono" />
								</div>
								<div class="space-y-2">
									<Label>เบอร์โทร</Label>
									<Input bind:value={fatherPhone} type="tel" />
								</div>
								<div class="space-y-2">
									<Label>อาชีพ</Label>
									<Input bind:value={fatherOccupation} />
								</div>
							</div>
						</div>

						<Separator />

						<!-- มารดา -->
						<div class="space-y-3">
							<p class="text-xs font-semibold uppercase tracking-wide text-gray-500">มารดา</p>
							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
								<div class="space-y-2">
									<Label>ชื่อ-นามสกุล</Label>
									<Input bind:value={motherName} placeholder="นางสมศรี ใจดี" />
								</div>
								<div class="space-y-2">
									<Label>เลขประชาชน</Label>
									<Input bind:value={motherNationalId} maxlength={13} class="font-mono" />
								</div>
								<div class="space-y-2">
									<Label>เบอร์โทร</Label>
									<Input bind:value={motherPhone} type="tel" />
								</div>
								<div class="space-y-2">
									<Label>อาชีพ</Label>
									<Input bind:value={motherOccupation} />
								</div>
							</div>
						</div>

						<Separator />

						<!-- ผู้ปกครอง -->
						<div class="space-y-3">
							<p class="text-xs font-semibold uppercase tracking-wide text-gray-500">
								ผู้ปกครอง <span class="normal-case font-normal text-gray-400"
									>(กรณีไม่ใช่บิดา-มารดา)</span
								>
							</p>
							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
								<div class="space-y-2">
									<Label>ชื่อ-นามสกุล ผู้ปกครอง</Label>
									<Input bind:value={guardianName} />
								</div>
								<div class="space-y-2">
									<Label>ความสัมพันธ์</Label>
									<Input bind:value={guardianRelation} placeholder="เช่น ปู่, ย่า, ลุง" />
								</div>
								<div class="space-y-2">
									<Label>เบอร์โทร</Label>
									<Input bind:value={guardianPhone} type="tel" />
								</div>
								<div class="space-y-2">
									<Label>เลขประชาชน</Label>
									<Input bind:value={guardianNationalId} maxlength={13} class="font-mono" />
								</div>
							</div>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- Submit -->
				<div class="flex flex-col sm:flex-row items-center justify-between gap-4 pt-2">
					<p class="text-xs text-gray-400 text-center sm:text-left max-w-sm">
						ข้าพเจ้าขอรับรองว่าข้อมูลที่กรอกทั้งหมดเป็นความจริง
					</p>
					<Button
						type="submit"
						size="lg"
						disabled={submitting}
						class="w-full sm:w-auto px-10 text-base"
					>
						{submitting ? '⏳ กำลังส่งข้อมูล...' : '📨 ส่งใบสมัคร'}
					</Button>
				</div>
			</form>
		{/if}
	</div>
</div>
