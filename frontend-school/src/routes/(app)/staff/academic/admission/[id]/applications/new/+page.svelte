<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { createApplication } from '$lib/api/admission';
	import { getAcademicStructure, type AcademicStructureData } from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import ClipboardList from 'lucide-svelte/icons/clipboard-list';

	let { data } = $props();
	const { periodId } = data;

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let loading = $state(true);
	let submitting = $state(false);

	// Form fields
	let title = $state('เด็กชาย');
	let firstName = $state('');
	let lastName = $state('');
	let nationalId = $state('');
	let dateOfBirth = $state('');
	let gender = $state('male');
	let nationality = $state('ไทย');
	let religion = $state('');
	let bloodType = $state('');
	let phone = $state('');
	let email = $state('');
	let address = $state('');
	let previousSchool = $state('');
	let previousGrade = $state('');
	let previousGpa = $state('');
	let gradeLevelId = $state('');
	let classPreference = $state('');
	let guardianName = $state('');
	let guardianRelationship = $state('บิดา');
	let guardianPhone = $state('');
	let guardianEmail = $state('');
	let guardianOccupation = $state('');
	let guardianNationalId = $state('');

	const titleOptions = ['เด็กชาย', 'เด็กหญิง', 'นาย', 'นางสาว', 'นาง'];
	const bloodTypes = ['A', 'B', 'AB', 'O'];

	async function loadData() {
		try {
			const res = await getAcademicStructure();
			structure = res.data;
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleSubmit() {
		if (!firstName || !lastName) {
			toast.error('กรุณากรอกชื่อ-นามสกุลผู้สมัคร');
			return;
		}
		submitting = true;
		try {
			await createApplication({
				admission_period_id: periodId,
				applicant_first_name: firstName,
				applicant_last_name: lastName,
				applicant_title: title || undefined,
				applicant_national_id: nationalId || undefined,
				applicant_date_of_birth: dateOfBirth || undefined,
				applicant_gender: gender || undefined,
				applicant_nationality: nationality || undefined,
				applicant_religion: religion || undefined,
				applicant_blood_type: bloodType || undefined,
				applicant_phone: phone || undefined,
				applicant_email: email || undefined,
				applicant_address: address || undefined,
				previous_school: previousSchool || undefined,
				previous_grade: previousGrade || undefined,
				previous_gpa: previousGpa ? parseFloat(previousGpa) : undefined,
				applying_grade_level_id: gradeLevelId || undefined,
				applying_classroom_preference: classPreference || undefined,
				guardian_name: guardianName || undefined,
				guardian_relationship: guardianRelationship || undefined,
				guardian_phone: guardianPhone || undefined,
				guardian_email: guardianEmail || undefined,
				guardian_occupation: guardianOccupation || undefined,
				guardian_national_id: guardianNationalId || undefined
			});
			toast.success('บันทึกใบสมัครเรียบร้อยแล้ว');
			goto(`/staff/academic/admission/${periodId}`);
		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="mx-auto max-w-4xl space-y-6">
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" href="/staff/academic/admission/{periodId}">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-bold">
				<ClipboardList class="h-6 w-6 text-primary" />
				เพิ่มใบสมัครนักเรียน
			</h1>
			<p class="text-sm text-muted-foreground">กรอกข้อมูลผู้สมัครและผู้ปกครอง</p>
		</div>
	</div>

	{#if loading}
		<div class="flex h-48 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="space-y-6">
			<!-- ข้อมูลผู้สมัคร -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ข้อมูลผู้สมัคร</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-2">
					<div class="grid gap-2">
						<Label>คำนำหน้า</Label>
						<Select.Root type="single" bind:value={title}>
							<Select.Trigger class="w-full">{title}</Select.Trigger>
							<Select.Content>
								{#each titleOptions as t}
									<Select.Item value={t}>{t}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label>เพศ</Label>
						<Select.Root type="single" bind:value={gender}>
							<Select.Trigger class="w-full">{gender === 'male' ? 'ชาย' : 'หญิง'}</Select.Trigger>
							<Select.Content>
								<Select.Item value="male">ชาย</Select.Item>
								<Select.Item value="female">หญิง</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label for="fn">ชื่อ <span class="text-red-500">*</span></Label>
						<Input id="fn" bind:value={firstName} placeholder="ชื่อจริง" />
					</div>
					<div class="grid gap-2">
						<Label for="ln">นามสกุล <span class="text-red-500">*</span></Label>
						<Input id="ln" bind:value={lastName} placeholder="นามสกุล" />
					</div>
					<div class="grid gap-2">
						<Label for="nid">เลขบัตรประชาชน</Label>
						<Input id="nid" bind:value={nationalId} placeholder="1-XXXX-XXXXX-XX-X" />
					</div>
					<div class="grid gap-2">
						<Label for="dob">วันเกิด</Label>
						<Input id="dob" type="date" bind:value={dateOfBirth} />
					</div>
					<div class="grid gap-2">
						<Label for="nat">สัญชาติ</Label>
						<Input id="nat" bind:value={nationality} />
					</div>
					<div class="grid gap-2">
						<Label for="rel">ศาสนา</Label>
						<Input id="rel" bind:value={religion} placeholder="พุทธ, คริสต์, อิสลาม..." />
					</div>
					<div class="grid gap-2">
						<Label>หมู่โลหิต</Label>
						<Select.Root type="single" bind:value={bloodType}>
							<Select.Trigger class="w-full">{bloodType || 'ไม่ระบุ'}</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ไม่ระบุ</Select.Item>
								{#each bloodTypes as bt}
									<Select.Item value={bt}>{bt}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label for="ph">เบอร์โทร</Label>
						<Input id="ph" bind:value={phone} placeholder="0XX-XXX-XXXX" />
					</div>
					<div class="grid gap-2">
						<Label for="em">อีเมล</Label>
						<Input id="em" type="email" bind:value={email} placeholder="example@email.com" />
					</div>
					<div class="grid gap-2 sm:col-span-2">
						<Label for="addr">ที่อยู่</Label>
						<Textarea
							id="addr"
							bind:value={address}
							placeholder="บ้านเลขที่ ถนน ตำบล อำเภอ จังหวัด รหัสไปรษณีย์"
							rows={2}
						/>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- การศึกษาเดิมและระดับที่สมัคร -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ข้อมูลการศึกษาและการสมัคร</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-2">
					<div class="grid gap-2">
						<Label for="ps">โรงเรียนเดิม</Label>
						<Input id="ps" bind:value={previousSchool} placeholder="ชื่อโรงเรียนเดิม" />
					</div>
					<div class="grid gap-2">
						<Label for="pg">ชั้นที่กำลังเรียน/จบ</Label>
						<Input id="pg" bind:value={previousGrade} placeholder="เช่น ป.6, ม.3" />
					</div>
					<div class="grid gap-2">
						<Label for="gpa">ผลการเรียน GPA</Label>
						<Input
							id="gpa"
							type="number"
							bind:value={previousGpa}
							placeholder="0.00"
							min="0"
							max="4"
							step="0.01"
						/>
					</div>
					<div class="grid gap-2">
						<Label>ระดับชั้นที่สมัคร</Label>
						<Select.Root type="single" bind:value={gradeLevelId}>
							<Select.Trigger class="w-full">
								{structure.levels.find((l) => l.id === gradeLevelId)?.short_name ||
									'เลือกระดับชั้น'}
							</Select.Trigger>
							<Select.Content>
								{#each structure.levels as level}
									<Select.Item value={level.id}>{level.short_name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2 sm:col-span-2">
						<Label for="cp">ความต้องการพิเศษ (ถ้ามี)</Label>
						<Input id="cp" bind:value={classPreference} placeholder="เช่น ห้อง EP, ทั่วไป" />
					</div>
				</Card.Content>
			</Card.Root>

			<!-- ผู้ปกครอง -->
			<Card.Root>
				<Card.Header>
					<Card.Title>ข้อมูลผู้ปกครอง</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-4 sm:grid-cols-2">
					<div class="grid gap-2">
						<Label for="gn">ชื่อผู้ปกครอง</Label>
						<Input id="gn" bind:value={guardianName} placeholder="ชื่อ-นามสกุลผู้ปกครอง" />
					</div>
					<div class="grid gap-2">
						<Label for="gr">ความสัมพันธ์</Label>
						<Select.Root type="single" bind:value={guardianRelationship}>
							<Select.Trigger class="w-full">{guardianRelationship}</Select.Trigger>
							<Select.Content>
								{#each ['บิดา', 'มารดา', 'ปู่/ย่า', 'ตา/ยาย', 'พี่', 'อื่นๆ'] as r}
									<Select.Item value={r}>{r}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label for="gph">เบอร์โทร</Label>
						<Input id="gph" bind:value={guardianPhone} placeholder="0XX-XXX-XXXX" />
					</div>
					<div class="grid gap-2">
						<Label for="gem">อีเมล</Label>
						<Input id="gem" type="email" bind:value={guardianEmail} />
					</div>
					<div class="grid gap-2">
						<Label for="goc">อาชีพ</Label>
						<Input id="goc" bind:value={guardianOccupation} />
					</div>
					<div class="grid gap-2">
						<Label for="gnid">เลขบัตรปชช. ผู้ปกครอง</Label>
						<Input id="gnid" bind:value={guardianNationalId} placeholder="1-XXXX-XXXXX-XX-X" />
					</div>
				</Card.Content>
			</Card.Root>

			<div class="flex justify-end gap-3">
				<Button variant="outline" href="/staff/academic/admission/{periodId}">ยกเลิก</Button>
				<Button onclick={handleSubmit} disabled={submitting} class="min-w-32">
					{#if submitting}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
					บันทึกใบสมัคร
				</Button>
			</div>
		</div>
	{/if}
</div>
