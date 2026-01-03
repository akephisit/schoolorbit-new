<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		getStaffProfile,
		updateStaff,
		listRoles,
		listDepartments,
		type StaffProfileResponse,
		type Role,
		type Department
	} from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import {
		ArrowLeft,
		LoaderCircle,
		Save,
		User,
		Building2,
		BookOpen,
		Check
	} from 'lucide-svelte';
	import { onMount } from 'svelte';

	const staffId = $derived(page.params.id);

	// Form state
	let currentStep = $state(1);
	const totalSteps = 4;

	// Loading states
	let loadingProfile = $state(true);
	let saving = $state(false);
	let loadingRoles = $state(true);
	let loadingDepartments = $state(true);

	// Data
	let staff: StaffProfileResponse | null = $state(null);
	let roles: Role[] = $state([]);
	let departments: Department[] = $state([]);

	// Form data
	let formData = $state({
		// Personal Information
		title: '',
		first_name: '',
		last_name: '',
		nickname: '',
		email: '',
		phone: '',
		emergency_contact: '',
		line_id: '',
		date_of_birth: '',
		gender: 'male',
		address: '',
		hired_date: '',
		status: 'active',

		// Staff Info
		education_level: '',
		major: '',
		university: '',

		// Roles
		role_ids: [] as string[],
		primary_role_id: '',

		// Departments
		department_assignments: [] as Array<{
			department_id: string;
			position: string;
			is_primary: boolean;
			responsibilities: string;
		}>
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
					email: staff.email || '',
					phone: staff.phone || '',
					emergency_contact: staff.emergency_contact || '',
					line_id: staff.line_id || '',
					date_of_birth: staff.date_of_birth || '',
					gender: staff.gender || 'male',
					address: staff.address || '',
					hired_date: staff.hired_date || '',
					status: staff.status,
					education_level: staff.staff_info?.education_level || '',
					major: staff.staff_info?.major || '',
					university: staff.staff_info?.university || '',
					role_ids: staff.roles?.map((r) => r.id) || [],
					primary_role_id: staff.primary_role?.id || '',
					department_assignments:
						staff.departments?.map((d) => ({
							department_id: d.id,
							position: d.position || 'member',
							is_primary: d.is_primary || false,
							responsibilities: d.responsibilities || ''
						})) || []
				};
			}
		} catch (e) {
			console.error('Failed to load staff profile:', e);
			errors.load = 'ไม่สามารถโหลดข้อมูลบุคลากรได้';
		} finally {
			loadingProfile = false;
		}
	}

	// Load roles and departments
	async function loadOptions() {
		try {
			const [rolesRes, deptsRes] = await Promise.all([listRoles(), listDepartments()]);

			if (rolesRes.success && rolesRes.data) {
				roles = rolesRes.data;
			}
			if (deptsRes.success && deptsRes.data) {
				departments = deptsRes.data;
			}
		} catch (e) {
			console.error('Failed to load options:', e);
		} finally {
			loadingRoles = false;
			loadingDepartments = false;
		}
	}

	onMount(() => {
		loadStaffProfile();
		loadOptions();
	});

	// Validation functions
	function validateStep1(): boolean {
		errors = {};

		if (!formData.first_name) errors.first_name = 'กรุณากรอกชื่อ';
		if (!formData.last_name) errors.last_name = 'กรุณากรอกนามสกุล';

		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
		}

		if (formData.phone && !/^[0-9-]+$/.test(formData.phone)) {
			errors.phone = 'หมายเลขโทรศัพท์ไม่ถูกต้อง';
		}

		return Object.keys(errors).length === 0;
	}

	function validateStep2(): boolean {
		// Educational info is optional
		return true;
	}

	function validateStep3(): boolean {
		errors = {};

		if (formData.role_ids.length === 0) {
			errors.roles = 'กรุณาเลือกบทบาทอย่างน้อย 1 บทบาท';
			return false;
		}

		if (!formData.primary_role_id) {
			errors.primary_role = 'กรุณาเลือกบทบาทหลัก';
			return false;
		}

		return true;
	}

	function validateStep4(): boolean {
		errors = {};

		if (formData.department_assignments.length === 0) {
			errors.departments = 'กรุณาเพิ่มสังกัดฝ่ายอย่างน้อย 1 ฝ่าย';
			return false;
		}

		const hasPrimary = formData.department_assignments.some((d) => d.is_primary);
		if (!hasPrimary) {
			errors.departments = 'กรุณาระบุฝ่ายหลัก';
			return false;
		}

		return true;
	}

	// Navigation functions
	function nextStep() {
		let isValid = false;

		switch (currentStep) {
			case 1:
				isValid = validateStep1();
				break;
			case 2:
				isValid = validateStep2();
				break;
			case 3:
				isValid = validateStep3();
				break;
			case 4:
				isValid = validateStep4();
				break;
		}

		if (isValid && currentStep < totalSteps) {
			currentStep++;
		}
	}

	function prevStep() {
		if (currentStep > 1) {
			currentStep--;
		}
	}

	// Role management
	function toggleRole(roleId: string) {
		const index = formData.role_ids.indexOf(roleId);
		if (index === -1) {
			formData.role_ids = [...formData.role_ids, roleId];
			if (!formData.primary_role_id) {
				formData.primary_role_id = roleId;
			}
		} else {
			formData.role_ids = formData.role_ids.filter((id) => id !== roleId);
			if (formData.primary_role_id === roleId) {
				formData.primary_role_id = formData.role_ids[0] || '';
			}
		}
	}

	// Department management
	function addDepartment() {
		const isFirst = formData.department_assignments.length === 0;
		formData.department_assignments = [
			...formData.department_assignments,
			{
				department_id: '',
				position: 'member',
				is_primary: isFirst,
				responsibilities: ''
			}
		];
	}

	function removeDepartment(index: number) {
		formData.department_assignments = formData.department_assignments.filter((_, i) => i !== index);
	}

	function setPrimaryDepartment(index: number) {
		formData.department_assignments = formData.department_assignments.map((dept, i) => ({
			...dept,
			is_primary: i === index
		}));
	}

	// Submit form
	async function handleSubmit() {
		if (!validateStep4()) return;
		if (!staffId) return;

		saving = true;
		errors = {};
		successMessage = '';

		try {
			const payload = {
				title: formData.title || undefined,
				first_name: formData.first_name,
				last_name: formData.last_name,
				nickname: formData.nickname || undefined,
				email: formData.email || undefined,
				phone: formData.phone || undefined,
				emergency_contact: formData.emergency_contact || undefined,
				line_id: formData.line_id || undefined,
				date_of_birth: formData.date_of_birth || undefined,
				gender: formData.gender || undefined,
				address: formData.address || undefined,
				hired_date: formData.hired_date || undefined,
				status: formData.status,
				staff_info: {
					education_level: formData.education_level || undefined,
					major: formData.major || undefined,
					university: formData.university || undefined
				},
				role_ids: formData.role_ids,
				primary_role_id: formData.primary_role_id || formData.role_ids[0],
				department_assignments: formData.department_assignments.filter((d) => d.department_id)
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

	function getGenderLabel(value: string): string {
		const labels: Record<string, string> = {
			male: 'ชาย',
			female: 'หญิง',
			other: 'อื่นๆ'
		};
		return labels[value] || 'เลือกเพศ';
	}

	function getPositionLabel(value: string): string {
		const labels: Record<string, string> = {
			member: 'สมาชิก',
			coordinator: 'ผู้ประสานงาน',
			deputy_head: 'รองหัวหน้าฝ่าย',
			head: 'หัวหน้าฝ่าย'
		};
		return labels[value] || 'เลือกตำแหน่ง';
	}

	// Get step icon
	function getStepIcon(step: number) {
		switch (step) {
			case 1:
				return User;
			case 2:
				return BookOpen; // Education
			case 3:
				return Building2; // Roles
			case 4:
				return Building2; // Departments
			default:
				return User;
		}
	}
</script>

<svelte:head>
	<title>
		{staff ? `แก้ไข ${staff.first_name} ${staff.last_name}` : 'แก้ไขบุคลากร'} - SchoolOrbit
	</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button href="/staff/{staffId}" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4" />
		</Button>
		<div>
			<h1 class="text-2xl font-bold text-foreground">แก้ไขข้อมูลบุคลากร</h1>
			{#if staff}
				<p class="text-sm text-muted-foreground">
					{staff.first_name}
					{staff.last_name} • ขั้นตอน {currentStep} / {totalSteps}
				</p>
			{/if}
		</div>
	</div>

	<div class="space-y-6">
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
			<!-- Progress Steps -->
			<div class="mb-8">
				<div class="flex items-center justify-between">
					{#each Array(totalSteps) as _, i (i)}
						<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
						{@const step = i + 1}
						{@const Icon = getStepIcon(step)}
						<div class="flex flex-col items-center flex-1">
							<!-- Circle -->
							<div
								class="w-12 h-12 rounded-full flex items-center justify-center transition-all
								{step < currentStep
									? 'bg-primary text-primary-foreground'
									: step === currentStep
										? 'bg-primary text-primary-foreground ring-4 ring-primary/20'
										: 'bg-muted text-muted-foreground'}"
							>
								{#if step < currentStep}
									<Check class="w-6 h-6" />
								{:else}
									<Icon class="w-6 h-6" />
								{/if}
							</div>

							<!-- Line (except last) -->
							{#if i < totalSteps - 1}
								<div
									class="absolute left-1/2 w-full h-0.5 top-6 -z-10
									{step < currentStep ? 'bg-primary' : 'bg-border'}"
									style="width: calc(100% / {totalSteps} - 3rem); transform: translateX(1.5rem);"
								></div>
							{/if}

							<!-- Label -->
							<p
								class="text-xs mt-2 text-center
								{step === currentStep ? 'text-foreground font-medium' : 'text-muted-foreground'}"
							>
								{#if step === 1}
									ข้อมูลส่วนตัว
								{:else if step === 2}
									การศึกษา
								{:else if step === 3}
									บทบาท
								{:else}
									สังกัดฝ่าย
								{/if}
							</p>
						</div>
					{/each}
				</div>
			</div>

			<form
				onsubmit={(e) => {
					e.preventDefault();
					if (currentStep === totalSteps) {
						handleSubmit();
					}
				}}
			>
				<div class="bg-card border border-border rounded-lg p-6">
					{#if currentStep === 1}
						<!-- Step 1: Personal Information -->
						<h2 class="text-xl font-semibold mb-6">ข้อมูลส่วนตัว</h2>

						<div class="space-y-4">
							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">
										คำนำหน้า <span class="text-destructive">*</span>
									</Label>
									<Select.Root type="single" bind:value={formData.title}>
										<Select.Trigger>{getTitleLabel(formData.title)}</Select.Trigger>
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

								<div>
									<Label class="mb-2">
										เพศ <span class="text-destructive">*</span>
									</Label>
									<Select.Root type="single" bind:value={formData.gender}>
										<Select.Trigger>{getGenderLabel(formData.gender)}</Select.Trigger>
										<Select.Content>
											<Select.Item value="male">ชาย</Select.Item>
											<Select.Item value="female">หญิง</Select.Item>
											<Select.Item value="other">อื่นๆ</Select.Item>
										</Select.Content>
									</Select.Root>
								</div>
							</div>

							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">
										ชื่อ <span class="text-destructive">*</span>
									</Label>
									<Input
										type="text"
										bind:value={formData.first_name}
										placeholder="ชื่อ"
										class="w-full px-3 py-2 border border-border rounded-md
										{errors.first_name ? 'border-destructive' : ''}"
									/>
									{#if errors.first_name}
										<p class="text-xs text-destructive mt-1">{errors.first_name}</p>
									{/if}
								</div>

								<div>
									<Label class="mb-2">
										นามสกุล <span class="text-destructive">*</span>
									</Label>
									<Input
										type="text"
										bind:value={formData.last_name}
										placeholder="นามสกุล"
										class="w-full px-3 py-2 border border-border rounded-md
										{errors.last_name ? 'border-destructive' : ''}"
									/>
									{#if errors.last_name}
										<p class="text-xs text-destructive mt-1">{errors.last_name}</p>
									{/if}
								</div>
							</div>

							<div>
								<Label class="mb-2">ชื่อเล่น</Label>
								<Input type="text" bind:value={formData.nickname} placeholder="ชื่อเล่น" />
							</div>

							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">อีเมล</Label>
									<Input
										type="email"
										bind:value={formData.email}
										placeholder="email@school.ac.th (ไม่บังคับ)"
										class="w-full px-3 py-2 border border-border rounded-md
										{errors.email ? 'border-destructive' : ''}"
									/>
									{#if errors.email}
										<p class="text-xs text-destructive mt-1">{errors.email}</p>
									{/if}
								</div>

								<div>
									<Label class="mb-2">หมายเลขโทรศัพท์</Label>
									<Input
										type="tel"
										bind:value={formData.phone}
										placeholder="081-234-5678"
										class="w-full px-3 py-2 border border-border rounded-md
										{errors.phone ? 'border-destructive' : ''}"
									/>
									{#if errors.phone}
										<p class="text-xs text-destructive mt-1">{errors.phone}</p>
									{/if}
								</div>
							</div>

							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">วันเกิด</Label>
									<DatePicker bind:value={formData.date_of_birth} placeholder="เลือกวันเกิด" />
								</div>

								<div>
									<Label class="mb-2">วันที่เริ่มงาน</Label>
									<DatePicker bind:value={formData.hired_date} placeholder="เลือกวันที่เริ่มงาน" />
								</div>
							</div>

							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">Line ID</Label>
									<Input type="text" bind:value={formData.line_id} placeholder="@lineid" />
								</div>

								<div>
									<Label class="mb-2">เบอร์ติดต่อฉุกเฉิน</Label>
									<Input
										type="tel"
										bind:value={formData.emergency_contact}
										placeholder="081-234-5678"
									/>
								</div>
							</div>

							<div>
								<Label class="mb-2">ที่อยู่</Label>
								<Textarea bind:value={formData.address} placeholder="ที่อยู่ปัจจุบัน" rows={3} />
							</div>

							<div>
								<Label class="mb-2">สถานะ</Label>
								<Select.Root type="single" bind:value={formData.status}>
									<Select.Trigger>{getStatusLabel(formData.status)}</Select.Trigger>
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
					{:else if currentStep === 2}
						<!-- Step 2: Educational Background -->
						<h2 class="text-xl font-semibold mb-6">ข้อมูลการศึกษา</h2>

						<div class="space-y-4">
							<div>
								<Label class="mb-2">วุฒิการศึกษา</Label>
								<Input
									type="text"
									bind:value={formData.education_level}
									placeholder="เช่น ปริญญาตรี, ปริญญาโท"
								/>
							</div>

							<div class="grid grid-cols-2 gap-4">
								<div>
									<Label class="mb-2">สาขา</Label>
									<Input
										type="text"
										bind:value={formData.major}
										placeholder="เช่น การศึกษา, คณิตศาสตร์"
									/>
								</div>

								<div>
									<Label class="mb-2">สถาบัน</Label>
									<Input
										type="text"
										bind:value={formData.university}
										placeholder="เช่น มหาวิทยาลัยธรรมศาสตร์"
									/>
								</div>
							</div>
						</div>
					{:else if currentStep === 3}
						<!-- Step 3: Roles -->
						<h2 class="text-xl font-semibold mb-6">บทบาทและตำแหน่ง</h2>

						{#if loadingRoles}
							<div class="flex justify-center py-8">
								<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
							</div>
						{:else}
							<div class="space-y-4">
								<p class="text-sm text-muted-foreground">
									เลือกบทบาทของบุคลากร (สามารถเลือกได้มากกว่า 1 บทบาท)
								</p>

								{#if errors.roles}
									<p class="text-sm text-destructive">{errors.roles}</p>
								{/if}

								<div class="grid grid-cols-2 gap-3">
									{#each roles as role (role.id)}
										<Button
											variant="outline"
											type="button"
											onclick={() => toggleRole(role.id)}
											class="p-4 border-2 rounded-lg text-left transition-all
											{formData.role_ids.includes(role.id)
												? 'border-primary bg-primary/5'
												: 'border-border hover:border-primary/50'}"
										>
											<div class="flex items-start justify-between mb-2">
												<div class="flex-1">
													<p class="font-medium">{role.name}</p>
													{#if role.name_en}
														<p class="text-xs text-muted-foreground">{role.name_en}</p>
													{/if}
												</div>
												<div class="flex gap-1">
													{#if formData.role_ids.includes(role.id)}
														<Check class="w-5 h-5 text-primary" />
													{/if}
												</div>
											</div>
											<div class="flex items-center gap-2 text-xs">
												<span class="px-2 py-0.5 bg-muted rounded">{role.category}</span>
												<span class="text-muted-foreground">ระดับ {role.level}</span>
											</div>
										</Button>
									{/each}
								</div>

								{#if formData.role_ids.length > 0}
									<div class="mt-6">
										<Label class="mb-2">
											บทบาทหลัก <span class="text-destructive">*</span>
										</Label>
										{#if errors.primary_role}
											<p class="text-sm text-destructive mb-2">{errors.primary_role}</p>
										{/if}
										<Select.Root type="single" bind:value={formData.primary_role_id}>
											<Select.Trigger>
												{#if formData.primary_role_id}
													{roles.find((r) => r.id === formData.primary_role_id)?.name ||
														'เลือกบทบาทหลัก'}
												{:else}
													เลือกบทบาทหลัก
												{/if}
											</Select.Trigger>
											<Select.Content>
												<Select.Item value="">เลือกบทบาทหลัก</Select.Item>
												{#each formData.role_ids as roleId (roleId)}
													{@const role = roles.find((r) => r.id === roleId)}
													{#if role}
														<Select.Item value={role.id}>{role.name}</Select.Item>
													{/if}
												{/each}
											</Select.Content>
										</Select.Root>
									</div>
								{/if}
							</div>
						{/if}
					{:else if currentStep === 4}
						<!-- Step 4: Departments -->
						<h2 class="text-xl font-semibold mb-6">สังกัดฝ่าย</h2>

						{#if loadingDepartments}
							<div class="flex justify-center py-8">
								<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
							</div>
						{:else}
							<div class="space-y-4">
								<p class="text-sm text-muted-foreground">
									ระบุฝ่ายที่บุคลากรสังกัดและตำแหน่งในฝ่าย
								</p>

								{#if errors.departments}
									<p class="text-sm text-destructive">{errors.departments}</p>
								{/if}

								{#each formData.department_assignments as dept, i (i)}
									<div class="p-4 border border-border rounded-lg">
										<div class="flex items-start justify-between mb-4">
											<h3 class="font-medium">ฝ่ายที่ {i + 1}</h3>
											{#if formData.department_assignments.length > 1}
												<Button
													variant="ghost"
													size="sm"
													type="button"
													onclick={() => removeDepartment(i)}
													class="text-destructive hover:text-destructive/80 text-sm"
												>
													ลบ
												</Button>
											{/if}
										</div>

										<div class="space-y-3">
											<div>
												<Label class="mb-2">ชื่อฝ่าย</Label>
												<Select.Root type="single" bind:value={dept.department_id}>
													<Select.Trigger>
														{#if dept.department_id}
															{departments.find((d) => d.id === dept.department_id)?.name ||
																'เลือกฝ่าย'}
														{:else}
															เลือกฝ่าย
														{/if}
													</Select.Trigger>
													<Select.Content>
														<Select.Item value="">เลือกฝ่าย</Select.Item>
														{#each departments as department (department.id)}
															<Select.Item value={department.id}>{department.name}</Select.Item>
														{/each}
													</Select.Content>
												</Select.Root>
											</div>

											<div>
												<Label class="mb-2">ตำแหน่งในฝ่าย</Label>
												<Select.Root type="single" bind:value={dept.position}>
													<Select.Trigger>{getPositionLabel(dept.position)}</Select.Trigger>
													<Select.Content>
														<Select.Item value="member">สมาชิก</Select.Item>
														<Select.Item value="coordinator">ผู้ประสานงาน</Select.Item>
														<Select.Item value="deputy_head">รองหัวหน้าฝ่าย</Select.Item>
														<Select.Item value="head">หัวหน้าฝ่าย</Select.Item>
													</Select.Content>
												</Select.Root>
											</div>

											<div>
												<Label class="mb-2">หน้าที่รับผิดชอบ</Label>
												<Textarea
													bind:value={dept.responsibilities}
													placeholder="ระบุหน้าที่รับผิดชอบ..."
													rows={2}
												/>
											</div>

											<div>
												<div class="flex items-center gap-2 cursor-pointer">
													<Checkbox
														checked={dept.is_primary}
														onCheckedChange={() => setPrimaryDepartment(i)}
													/>
													<span class="text-sm">ฝ่ายหลัก</span>
												</div>
											</div>
										</div>
									</div>
								{/each}

								<Button type="button" onclick={addDepartment} variant="outline" class="w-full">
									+ เพิ่มฝ่าย
								</Button>
							</div>
						{/if}
					{/if}
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

				<!-- Navigation Buttons -->
				<div class="flex justify-between mt-6">
					<Button
						type="button"
						onclick={prevStep}
						variant="outline"
						disabled={currentStep === 1}
						class="min-w-[120px]"
					>
						<ArrowLeft class="w-4 h-4 mr-2" />
						ย้อนกลับ
					</Button>

					{#if currentStep < totalSteps}
						<Button type="button" onclick={nextStep} class="min-w-[120px]">
							ถัดไป
							<ArrowLeft class="w-4 h-4 ml-2 rotate-180" />
						</Button>
					{:else}
						<Button type="submit" disabled={saving} class="min-w-[120px]">
							{#if saving}
								<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />
								กำลังบันทึก...
							{:else}
								<Save class="w-4 h-4 mr-2" />
								บันทึกการเปลี่ยนแปลง
							{/if}
						</Button>
					{/if}
				</div>
			</form>
		{/if}
	</div>
</div>
