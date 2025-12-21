<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { getStaffProfile, updateStaff, type StaffProfileResponse } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ArrowLeft, LoaderCircle, Save } from 'lucide-svelte';
	import { onMount } from 'svelte';

	const staffId = $derived(page.params.id);

	// Loading states
	let loadingProfile = $state(true);
	let saving = $state(false);

	// Data
	let staff: StaffProfileResponse | null = $state(null);

	// Form data
	let formData = $state({
		title: '',
		first_name: '',
		last_name: '',
		nickname: '',
		phone: '',
		status: 'active',
		// Staff Info
		education_level: '',
		major: '',
		university: ''
	});

	// Validation errors
	let errors = $state<Record<string, string>>({});
	let successMessage = $state('');

	// Load staff profile
	async function loadStaffProfile() {
		if (!staffId) return;

		try {
			loadingProfile = true;
			const response = await getStaffProfile(staffId);

			if (response.success && response.data) {
				staff = response.data;

				// Populate form
				formData = {
					title: staff.title || 'นาย',
					first_name: staff.first_name,
					last_name: staff.last_name,
					nickname: staff.nickname || '',
					phone: staff.phone || '',
					status: staff.status,
					// Staff Info
					education_level: staff.staff_info?.education_level || '',
					major: staff.staff_info?.major || '',
					university: staff.staff_info?.university || ''
				};
			}
		} catch (e) {
			console.error('Failed to load staff profile:', e);
			errors.load = 'ไม่สามารถโหลดข้อมูลบุคลากรได้';
		} finally {
			loadingProfile = false;
		}
	}

	onMount(() => {
		loadStaffProfile();
	});

	// Validation
	function validate(): boolean {
		errors = {};

		if (!formData.first_name) errors.first_name = 'กรุณากรอกชื่อ';
		if (!formData.last_name) errors.last_name = 'กรุณากรอกนามสกุล';
		if (formData.phone && !/^[0-9-]+$/.test(formData.phone)) {
			errors.phone = 'หมายเลขโทรศัพท์ไม่ถูกต้อง';
		}

		return Object.keys(errors).length === 0;
	}

	// Submit form
	async function handleSubmit() {
		if (!validate()) return;
		if (!staffId) return;

		saving = true;
		errors = {};
		successMessage = '';

		try {
			const payload = {
				title: formData.title,
				first_name: formData.first_name,
				last_name: formData.last_name,
				nickname: formData.nickname || undefined,
				phone: formData.phone || undefined,
				status: formData.status,
				staff_info: {
					education_level: formData.education_level || undefined,
					major: formData.major || undefined,
					university: formData.university || undefined
				}
			};

			const result = await updateStaff(staffId, payload);

			if (result.success) {
				successMessage = 'บันทึกข้อมูลสำเร็จ';
				setTimeout(async () => {
					// eslint-disable-next-line @typescript-eslint/no-explicit-any
					await goto(resolve(`/staff/${staffId}` as any), { invalidateAll: true });
				}, 1500);
			} else {
				errors.submit = result.error || 'เกิดข้อผิดพลาดในการบันทึกข้อมูล';
			}
		} catch (e) {
			errors.submit = e instanceof Error ? e.message : 'เกิดข้อผิดพลาดในการบันทึกข้อมูล';
		} finally {
			saving = false;
		}
	}

	// Get display label for value
	function getTitleLabel(value: string): string {
		const labels: Record<string, string> = {
			นาย: 'นาย',
			นาง: 'นาง',
			นางสาว: 'นางสาว',
			'ดร.': 'ดร.',
			'ศ.': 'ศ.',
			'รศ.': 'รศ.',
			'ผศ.': 'ผศ.'
		};
		return labels[value] || value;
	}

	function getStatusLabel(value: string): string {
		const labels: Record<string, string> = {
			active: 'ใช้งาน',
			inactive: 'ไม่ใช้งาน',
			suspended: 'ระงับ',
			resigned: 'ลาออก',
			retired: 'เกษียณ'
		};
		return labels[value] || value;
	}
</script>

<svelte:head>
	<title>
		{staff ? `แก้ไข ${staff.first_name} ${staff.last_name}` : 'แก้ไขบุคลากร'} - SchoolOrbit
	</title>
</svelte:head>

<div class="min-h-screen bg-background pb-12">
	<!-- Header -->
	<div class="bg-card border-b border-border sticky top-0 z-10">
		<div class="container max-w-4xl mx-auto px-4 py-4">
			<div class="flex items-center gap-4">
				<Button href="/staff/{staffId}" variant="ghost" size="sm">
					<ArrowLeft class="w-4 h-4" />
				</Button>
				<div>
					<h1 class="text-2xl font-bold text-foreground">แก้ไขข้อมูลบุคลากร</h1>
					{#if staff}
						<p class="text-sm text-muted-foreground">
							{staff.first_name}
							{staff.last_name}
						</p>
					{/if}
				</div>
			</div>
		</div>
	</div>

	<div class="container max-w-4xl mx-auto px-4 py-8">
		{#if loadingProfile}
			<div class="bg-card border border-border rounded-lg p-12 text-center">
				<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground mx-auto mb-4" />
				<p class="text-muted-foreground">กำลังโหลดข้อมูล...</p>
			</div>
		{:else if errors.load}
			<div class="bg-destructive/10 border border-destructive/20 rounded-lg p-6 text-center">
				<p class="text-destructive">{errors.load}</p>
				<Button onclick={loadStaffProfile} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
			</div>
		{:else if staff}
			<form
				onsubmit={(e) => {
					e.preventDefault();
					handleSubmit();
				}}
			>
				<div class="bg-card border border-border rounded-lg p-6">
					<h2 class="text-xl font-semibold mb-6">ข้อมูลส่วนตัว</h2>

					<div class="space-y-4">
						<div class="grid grid-cols-2 gap-4">
							<div class="space-y-2">
								<Label for="title">คำนำหน้า <span class="text-destructive">*</span></Label>
								<Select.Root type="single" bind:value={formData.title}>
									<Select.Trigger id="title">
										{getTitleLabel(formData.title)}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="นาย">นาย</Select.Item>
										<Select.Item value="นาง">นาง</Select.Item>
										<Select.Item value="นางสาว">นางสาว</Select.Item>
										<Select.Item value="ดร.">ดร.</Select.Item>
										<Select.Item value="ศ.">ศ.</Select.Item>
										<Select.Item value="รศ.">รศ.</Select.Item>
										<Select.Item value="ผศ.">ผศ.</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>

							<div class="space-y-2">
								<Label for="status">สถานะ</Label>
								<Select.Root type="single" bind:value={formData.status}>
									<Select.Trigger id="status">
										{getStatusLabel(formData.status)}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="active">ใช้งาน</Select.Item>
										<Select.Item value="inactive">ไม่ใช้งาน</Select.Item>
										<Select.Item value="suspended">ระงับ</Select.Item>
										<Select.Item value="resigned">ลาออก</Select.Item>
										<Select.Item value="retired">เกษียณ</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>
						</div>

						<div class="grid grid-cols-2 gap-4">
							<div class="space-y-2">
								<Label for="first_name">ชื่อ <span class="text-destructive">*</span></Label>
								<Input
									id="first_name"
									type="text"
									bind:value={formData.first_name}
									placeholder="ชื่อ"
									class={errors.first_name ? 'border-destructive' : ''}
								/>
								{#if errors.first_name}
									<p class="text-xs text-destructive">{errors.first_name}</p>
								{/if}
							</div>

							<div class="space-y-2">
								<Label for="last_name">นามสกุล <span class="text-destructive">*</span></Label>
								<Input
									id="last_name"
									type="text"
									bind:value={formData.last_name}
									placeholder="นามสกุล"
									class={errors.last_name ? 'border-destructive' : ''}
								/>
								{#if errors.last_name}
									<p class="text-xs text-destructive">{errors.last_name}</p>
								{/if}
							</div>
						</div>

						<div class="space-y-2">
							<Label for="nickname">ชื่อเล่น</Label>
							<Input
								id="nickname"
								type="text"
								bind:value={formData.nickname}
								placeholder="ชื่อเล่น"
							/>
						</div>

						<div class="space-y-2">
							<Label for="phone">หมายเลขโทรศัพท์</Label>
							<Input
								id="phone"
								type="tel"
								bind:value={formData.phone}
								placeholder="081-234-5678"
								class={errors.phone ? 'border-destructive' : ''}
							/>
							{#if errors.phone}
								<p class="text-xs text-destructive">{errors.phone}</p>
							{/if}
						</div>
					</div>
				</div>

				<h2 class="text-xl font-semibold mb-6 mt-8 pt-6 border-t border-border">ข้อมูลการทำงาน</h2>

				<div class="space-y-4">
					<div class="space-y-2">
						<Label for="education_level">วุฒิการศึกษา</Label>
						<Input
							id="education_level"
							type="text"
							bind:value={formData.education_level}
							placeholder="เช่น ปริญญาตรี, ปริญญาโท"
						/>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="major">สาขา</Label>
							<Input
								id="major"
								type="text"
								bind:value={formData.major}
								placeholder="เช่น การศึกษา, คณิตศาสตร์"
							/>
						</div>

						<div class="space-y-2">
							<Label for="university">สถาบัน</Label>
							<Input
								id="university"
								type="text"
								bind:value={formData.university}
								placeholder="เช่น มหาวิทยาลัยธรรมศาสตร์"
							/>
						</div>
					</div>

					<div class="bg-muted/50 p-4 rounded-lg">
						<p class="text-sm text-muted-foreground">
							<strong>หมายเหตุ:</strong> การแก้ไขบทบาท, ฝ่าย และวิชาที่สอน กรุณาติดต่อผู้ดูแลระบบ
						</p>
					</div>
				</div>

				<!-- Success Message -->
				{#if successMessage}
					<div class="mt-6 p-4 bg-green-100 border border-green-200 rounded-lg">
						<p class="text-sm text-green-800">{successMessage}</p>
					</div>
				{/if}

				<!-- Error Message -->
				{#if errors.submit}
					<div class="mt-6 p-4 bg-destructive/10 border border-destructive/20 rounded-lg">
						<p class="text-sm text-destructive">{errors.submit}</p>
					</div>
				{/if}

				<!-- Action Buttons -->
				<div class="flex justify-between mt-6">
					<Button href="/staff/{staffId}" variant="outline">
						<ArrowLeft class="w-4 h-4 mr-2" />
						ยกเลิก
					</Button>

					<Button type="submit" disabled={saving}>
						{#if saving}
							<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />
							กำลังบันทึก...
						{:else}
							<Save class="w-4 h-4 mr-2" />
							บันทึกการเปลี่ยนแปลง
						{/if}
					</Button>
				</div>
			</form>
		{/if}
	</div>
</div>
