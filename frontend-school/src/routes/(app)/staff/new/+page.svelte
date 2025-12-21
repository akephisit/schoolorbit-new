<script lang="ts">
	import { goto } from '$app/navigation';
	import { createStaff, listRoles, listDepartments, type Role, type Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	
	import {
		User,
		Briefcase,
		Building2,
		BookOpen,
		ArrowLeft,
		ArrowRight,
		Check,
		Loader2
	} from 'lucide-svelte';
	import { onMount } from 'svelte';

	// Form state
	let currentStep = $state(1);
	const totalSteps = 4;

	// Loading states
	let loading = $state(false);
	let loadingRoles = $state(true);
	let loadingDepartments = $state(true);

	// Available options
	let roles: Role[] = $state([]);
	let departments: Department[] = $state([]);

	// Form data
	let formData = $state({
		// Step 1: Personal Information
		national_id: '',
		email: '',
		password: '',
		confirmPassword: '',
		title: 'นาย',
		first_name: '',
		last_name: '',
		nickname: '',
		phone: '',
		emergency_contact: '',
		line_id: '',
		date_of_birth: '',
		gender: 'male',
		address: '',
		hired_date: new Date().toISOString().split('T')[0],

		// Step 2: Staff Information
		staff_info: {
			employee_id: '',
			employment_type: 'permanent',
			education_level: '',
			major: '',
			university: '',
			teaching_license_number: '',
			teaching_license_expiry: '',
			work_days: ['monday', 'tuesday', 'wednesday', 'thursday', 'friday']
		},

		// Step 3: Roles
		role_ids: [] as string[],
		primary_role_id: '',

		// Step 4: Departments
		department_assignments: [] as Array<{
			department_id: string;
			position: string;
			is_primary: boolean;
			responsibilities: string;
		}>
	});

	// Validation errors
	let errors = $state<Record<string, string>>({});

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
		loadOptions();
		// Load draft from localStorage
		const draft = localStorage.getItem('staff-create-draft');
		if (draft) {
			try {
				formData = JSON.parse(draft);
			} catch (e) {
				console.error('Failed to load draft:', e);
			}
		}
	});

	// Save draft to localStorage
	function saveDraft() {
		localStorage.setItem('staff-create-draft', JSON.stringify(formData));
	}

	// Validation functions
	function validateStep1(): boolean {
		errors = {};

		if (!formData.first_name) errors.first_name = 'กรุณากรอกชื่อ';
		if (!formData.last_name) errors.last_name = 'กรุณากรอกนามสกุล';
		if (!formData.email) {
			errors.email = 'กรุณากรอกอีเมล';
		} else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
		}
		if (!formData.password) {
			errors.password = 'กรุณากรอกรหัสผ่าน';
		} else if (formData.password.length < 6) {
			errors.password = 'รหัสผ่านต้องมีอย่างน้อย 6 ตัวอักษร';
		}
		if (formData.password !== formData.confirmPassword) {
			errors.confirmPassword = 'รหัสผ่านไม่ตรงกัน';
		}
		if (formData.phone && !/^[0-9-]+$/.test(formData.phone)) {
			errors.phone = 'หมายเลขโทรศัพท์ไม่ถูกต้อง';
		}

		return Object.keys(errors).length === 0;
	}

	function validateStep2(): boolean {
		errors = {};
		// Optional fields, no strict validation needed
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
			saveDraft();
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

		loading = true;
		errors = {};

		try {
			const payload = {
				...formData,
				role_ids: formData.role_ids,
				primary_role_id: formData.primary_role_id || formData.role_ids[0],
				department_assignments: formData.department_assignments.filter((d) => d.department_id)
			};

			// Remove confirmPassword
			delete (payload as any).confirmPassword;

			const result = await createStaff(payload);

			if (result.success && result.data) {
				// Clear draft
				localStorage.removeItem('staff-create-draft');
				// Redirect to profile
				goto(`/staff/${result.data.id}`);
			} else {
				errors.submit = result.error || 'เกิดข้อผิดพลาดในการสร้างบุคลากร';
			}
		} catch (e) {
			errors.submit = e instanceof Error ? e.message : 'เกิดข้อผิดพลาดในการสร้างบุคลากร';
		} finally {
			loading = false;
		}
	}

	// Get step icon
	function getStepIcon(step: number) {
		switch (step) {
			case 1:
				return User;
			case 2:
				return Briefcase;
			case 3:
				return Building2;
			case 4:
				return BookOpen;
			default:
				return User;
		}
	}
</script>

<svelte:head>
	<title>เพิ่มบุคลากร - SchoolOrbit</title>
</svelte:head>

<div class="min-h-screen bg-background pb-12">
	<!-- Header -->
	<div class="bg-card border-b border-border sticky top-0 z-10">
		<div class="container max-w-4xl mx-auto px-4 py-4">
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-4">
					<Button href="/staff" variant="ghost" size="sm">
						<ArrowLeft class="w-4 h-4" />
					</Button>
					<div>
						<h1 class="text-2xl font-bold text-foreground">เพิ่มบุคลากรใหม่</h1>
						<p class="text-sm text-muted-foreground">กรอกข้อมูลบุคลากรให้ครบถ้วน</p>
					</div>
				</div>
				<div class="text-sm text-muted-foreground">
					ขั้นตอน {currentStep} / {totalSteps}
				</div>
			</div>
		</div>
	</div>

	<div class="container max-w-4xl mx-auto px-4 py-8">
		<!-- Progress Steps -->
		<div class="mb-8">
			<div class="flex items-center justify-between">
				{#each Array(totalSteps) as _, i}
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
								ข้อมูลการทำงาน
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

		<!-- Form Content -->
		<div class="bg-card border border-border rounded-lg p-6">
			{#if currentStep === 1}
				<!-- Step 1: Personal Information -->
				<h2 class="text-xl font-semibold mb-6">ข้อมูลส่วนตัว</h2>

				<div class="space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">
								คำนำหน้า <span class="text-destructive">*</span>
							</label>
							<select
								bind:value={formData.title}
								
							>
								<option value="นาย">นาย</option>
								<option value="นาง">นาง</option>
								<option value="นางสาว">นางสาว</option>
								<option value="ดร.">ดร.</option>
								<option value="ศ.">ศ.</option>
								<option value="รศ.">รศ.</option>
								<option value="ผศ.">ผศ.</option>
							</select>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">
								เพศ <span class="text-destructive">*</span>
							</label>
							<select
								bind:value={formData.gender}
								
							>
								<option value="male">ชาย</option>
								<option value="female">หญิง</option>
								<option value="other">อื่นๆ</option>
							</select>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">
								ชื่อ <span class="text-destructive">*</span>
							</label>
							<Input type="text" bind:value={formData.first_name}
								placeholder="ชื่อ"
								class="w-full px-3 py-2 border border-border rounded-md
								{errors.first_name ? 'border-destructive' : ''}"
							/>
							{#if errors.first_name}
								<p class="text-xs text-destructive mt-1">{errors.first_name}</p>
							{/if}
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">
								นามสกุล <span class="text-destructive">*</span>
							</label>
							<Input type="text" bind:value={formData.last_name}
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
						<label class="block text-sm font-medium mb-2">ชื่อเล่น</label>
						<Input type="text" bind:value={formData.nickname}
							placeholder="ชื่อเล่น"
							
						/>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">
								อีเมล <span class="text-destructive">*</span>
							</label>
							<Input type="email" bind:value={formData.email}
								placeholder="email@school.ac.th"
								class="w-full px-3 py-2 border border-border rounded-md
								{errors.email ? 'border-destructive' : ''}"
							/>
							{#if errors.email}
								<p class="text-xs text-destructive mt-1">{errors.email}</p>
							{/if}
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">หมายเลขโทรศัพท์</label>
							<Input type="tel" bind:value={formData.phone}
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
							<label class="block text-sm font-medium mb-2">
								รหัสผ่าน <span class="text-destructive">*</span>
							</label>
							<Input type="password" bind:value={formData.password}
								placeholder="••••••••"
								class="w-full px-3 py-2 border border-border rounded-md
								{errors.password ? 'border-destructive' : ''}"
							/>
							{#if errors.password}
								<p class="text-xs text-destructive mt-1">{errors.password}</p>
							{/if}
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">
								ยืนยันรหัสผ่าน <span class="text-destructive">*</span>
							</label>
							<Input type="password" bind:value={formData.confirmPassword}
								placeholder="••••••••"
								class="w-full px-3 py-2 border border-border rounded-md
								{errors.confirmPassword ? 'border-destructive' : ''}"
							/>
							{#if errors.confirmPassword}
								<p class="text-xs text-destructive mt-1">{errors.confirmPassword}</p>
							{/if}
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">เลขบัตรประชาชน</label>
							<Input type="text" bind:value={formData.national_id}
								placeholder="1234567890123"
								maxlength={13}
								
							/>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">วันเกิด</label>
							<Input type="date" bind:value={formData.date_of_birth}
								
							/>
						</div>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">Line ID</label>
						<Input type="text" bind:value={formData.line_id}
							placeholder="@lineid"
							
						/>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">เบอร์ติดต่อฉุกเฉิน</label>
						<Input type="tel" bind:value={formData.emergency_contact}
							placeholder="081-234-5678"
							
						/>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">ที่อยู่</label>
						<Textarea bind:value={formData.address}
							placeholder="ที่อยู่ปัจจุบัน"
							rows={3}
							
						/>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">วันที่เริ่มงาน</label>
						<Input type="date" bind:value={formData.hired_date}
							
						/>
					</div>
				</div>
			{:else if currentStep === 2}
				<!-- Step 2: Staff Information -->
				<h2 class="text-xl font-semibold mb-6">ข้อมูลการทำงาน</h2>

				<div class="space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">รหัสพนักงาน</label>
							<Input type="text" bind:value={formData.staff_info.employee_id}
								placeholder="EMP001"
								
							/>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">ประเภทการจ้าง</label>
							<select
								bind:value={formData.staff_info.employment_type}
								
							>
								<option value="permanent">พนักงานประจำ</option>
								<option value="contract">พนักงานสัญญาจ้าง</option>
								<option value="temporary">พนักงานชั่วคราว</option>
								<option value="part_time">พนักงานพาร์ทไทม์</option>
							</select>
						</div>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">วุฒิการศึกษา</label>
						<Input type="text" bind:value={formData.staff_info.education_level}
							placeholder="ปริญญาตรี / ปริญญาโท / ปริญญาเอก"
							
						/>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">สาขาวิชา</label>
							<Input type="text" bind:value={formData.staff_info.major}
								placeholder="เช่น การศึกษา, วิศวกรรม"
								
							/>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">สถาบันการศึกษา</label>
							<Input type="text" bind:value={formData.staff_info.university}
								placeholder="มหาวิทยาลัย..."
								
							/>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<label class="block text-sm font-medium mb-2">เลขใบประกอบวิชาชีพครู</label>
							<Input type="text" bind:value={formData.staff_info.teaching_license_number}
								placeholder="TL123456"
								
							/>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">วันหมดอายุใบประกอบวิชาชีพ</label>
							<Input type="date" bind:value={formData.staff_info.teaching_license_expiry}
								
							/>
						</div>
					</div>

					<div>
						<label class="block text-sm font-medium mb-2">วันทำงาน</label>
						<div class="grid grid-cols-3 gap-2">
							{#each ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'] as day}
								<label class="flex items-center gap-2">
									<input
										type="checkbox"
										checked={formData.staff_info.work_days.includes(day)}
										onchange={(e) => {
											if (e.currentTarget.checked) {
												formData.staff_info.work_days = [...formData.staff_info.work_days, day];
											} else {
												formData.staff_info.work_days = formData.staff_info.work_days.filter(
													(d) => d !== day
												);
											}
										}}
										class="w-4 h-4"
									/>
									<span class="text-sm">
										{#if day === 'monday'}
											จันทร์
										{:else if day === 'tuesday'}
											อังคาร
										{:else if day === 'wednesday'}
											พุธ
										{:else if day === 'thursday'}
											พฤหัสบดี
										{:else if day === 'friday'}
											ศุกร์
										{:else if day === 'saturday'}
											เสาร์
										{:else}
											อาทิตย์
										{/if}
									</span>
								</label>
							{/each}
						</div>
					</div>
				</div>
			{:else if currentStep === 3}
				<!-- Step 3: Roles -->
				<h2 class="text-xl font-semibold mb-6">บทบาทและตำแหน่ง</h2>

				{#if loadingRoles}
					<div class="flex justify-center py-8">
						<Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
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
							{#each roles as role}
								<button
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
								</button>
							{/each}
						</div>

						{#if formData.role_ids.length > 0}
							<div class="mt-6">
								<label class="block text-sm font-medium mb-2">
									บทบาทหลัก <span class="text-destructive">*</span>
								</label>
								{#if errors.primary_role}
									<p class="text-sm text-destructive mb-2">{errors.primary_role}</p>
								{/if}
								<select
									bind:value={formData.primary_role_id}
									
								>
									<option value="">เลือกบทบาทหลัก</option>
									{#each formData.role_ids as roleId}
										{@const role = roles.find((r) => r.id === roleId)}
										{#if role}
											<option value={role.id}>{role.name}</option>
										{/if}
									{/each}
								</select>
							</div>
						{/if}
					</div>
				{/if}
			{:else if currentStep === 4}
				<!-- Step 4: Departments -->
				<h2 class="text-xl font-semibold mb-6">สังกัดฝ่าย</h2>

				{#if loadingDepartments}
					<div class="flex justify-center py-8">
						<Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
					</div>
				{:else}
					<div class="space-y-4">
						<p class="text-sm text-muted-foreground">ระบุฝ่ายที่บุคลากรสังกัดและตำแหน่งในฝ่าย</p>

						{#if errors.departments}
							<p class="text-sm text-destructive">{errors.departments}</p>
						{/if}

						{#each formData.department_assignments as dept, i}
							<div class="p-4 border border-border rounded-lg">
								<div class="flex items-start justify-between mb-4">
									<h3 class="font-medium">ฝ่ายที่ {i + 1}</h3>
									{#if formData.department_assignments.length > 1}
										<button
											type="button"
											onclick={() => removeDepartment(i)}
											class="text-destructive hover:text-destructive/80 text-sm"
										>
											ลบ
										</button>
									{/if}
								</div>

								<div class="space-y-3">
									<div>
										<label class="block text-sm font-medium mb-2">ชื่อฝ่าย</label>
										<select
											bind:value={dept.department_id}
											
										>
											<option value="">เลือกฝ่าย</option>
											{#each departments as department}
												<option value={department.id}>{department.name}</option>
											{/each}
										</select>
									</div>

									<div>
										<label class="block text-sm font-medium mb-2">ตำแหน่งในฝ่าย</label>
										<select
											bind:value={dept.position}
											
										>
											<option value="member">สมาชิก</option>
											<option value="coordinator">ผู้ประสานงาน</option>
											<option value="deputy_head">รองหัวหน้าฝ่าย</option>
											<option value="head">หัวหน้าฝ่าย</option>
										</select>
									</div>

									<div>
										<label class="block text-sm font-medium mb-2">หน้าที่รับผิดชอบ</label>
										<Textarea bind:value={dept.responsibilities}
											placeholder="ระบุหน้าที่รับผิดชอบ..."
											rows={2}
											
										/>
									</div>

									<div>
										<label class="flex items-center gap-2">
											<input
												type="radio"
												name="primary_dept"
												checked={dept.is_primary}
												onchange={() => setPrimaryDepartment(i)}
												class="w-4 h-4"
											/>
											<span class="text-sm">ฝ่ายหลัก</span>
										</label>
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
					<ArrowRight class="w-4 h-4 ml-2" />
				</Button>
			{:else}
				<Button type="button" onclick={handleSubmit} disabled={loading} class="min-w-[120px]">
					{#if loading}
						<Loader2 class="w-4 h-4 mr-2 animate-spin" />
						กำลังบันทึก...
					{:else}
						<Check class="w-4 h-4 mr-2" />
						สร้างบุคลากร
					{/if}
				</Button>
			{/if}
		</div>

		{#if errors.submit}
			<div class="mt-4 p-4 bg-destructive/10 border border-destructive/20 rounded-lg">
				<p class="text-sm text-destructive">{errors.submit}</p>
			</div>
		{/if}
	</div>
</div>

<style>
	input[type='checkbox']:checked {
		accent-color: hsl(var(--primary));
	}
	input[type='radio']:checked {
		accent-color: hsl(var(--primary));
	}
</style>
