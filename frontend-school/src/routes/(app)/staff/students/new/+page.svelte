<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Card } from '$lib/components/ui/card';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, User, Save, GraduationCap } from 'lucide-svelte';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Switch } from '$lib/components/ui/switch';
	import { createStudent } from '$lib/api/students';
	import {
		lookupGradeLevels,
		lookupClassrooms,
		type GradeLevelLookupItem,
		type ClassroomLookupItem
	} from '$lib/api/lookup';
	import { onMount } from 'svelte';

	// Form data
	let formData = $state({
		national_id: '',
		email: '',
		password: '',
		confirmPassword: '',
		title: 'เด็กชาย',
		first_name: '',
		last_name: '',
		student_id: '',
		grade_level: '',
		class_room: '',
		student_number: null as number | null,
		date_of_birth: '',
		gender: 'male',
		parent_enabled: false,
		parent: {
			first_name: '',
			last_name: '',
			phone: '',
			relationship: 'บิดา',
			national_id: '',
			email: ''
		}
	});

	let errors = $state<Record<string, string>>({});
	let loading = $state(false);

	// Dropdown Options
	let gradeLevels: GradeLevelLookupItem[] = $state([]);
	let classrooms: ClassroomLookupItem[] = $state([]);
	let selectedGradeId = $state('');

	let filteredClassrooms = $derived(
		selectedGradeId ? classrooms.filter((c) => c.grade_level_id === selectedGradeId) : classrooms
	);

	function handleGradeChange(id: string) {
		selectedGradeId = id;
		const gl = gradeLevels.find((g) => g.id === id);
		formData.grade_level = gl?.short_name || '';
		formData.class_room = ''; // Reset classroom when grade changes
	}

	onMount(async () => {
		try {
			const [gl, cr] = await Promise.all([lookupGradeLevels(), lookupClassrooms()]);
			gradeLevels = gl.sort((a, b) => a.level_order - b.level_order);
			classrooms = cr;
		} catch (e) {
			console.error('Failed to load options', e);
		}
	});

	function validateForm(): boolean {
		errors = {};

		// Optional fields check
		if (formData.national_id && !/^\d{13}$/.test(formData.national_id)) {
			errors.national_id = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก';
		}

		if (!formData.first_name) errors.first_name = 'กรุณากรอกชื่อ';
		if (!formData.last_name) errors.last_name = 'กรุณากรอกนามสกุล';
		if (!formData.student_id) errors.student_id = 'กรุณากรอกรหัสนักเรียน';

		// Password
		if (!formData.password) {
			errors.password = 'กรุณากรอกรหัสผ่าน';
		} else if (formData.password.length < 6) {
			errors.password = 'รหัสผ่านต้องมีอย่างน้อย 6 ตัวอักษร';
		}

		if (formData.password !== formData.confirmPassword) {
			errors.confirmPassword = 'รหัสผ่านไม่ตรงกัน';
		}

		// Email (optional but validate format)
		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
		}

		// Parent validation
		if (formData.parent_enabled) {
			if (!formData.parent.first_name) errors['parent.first_name'] = 'กรุณากรอกชื่อผู้ปกครอง';
			if (!formData.parent.last_name) errors['parent.last_name'] = 'กรุณากรอกนามสกุลผู้ปกครอง';
			if (!formData.parent.phone) errors['parent.phone'] = 'กรุณากรอกเบอร์โทรศัพท์';
			if (!/^\d{9,10}$/.test(formData.parent.phone))
				errors['parent.phone'] = 'เบอร์โทรศัพท์ไม่ถูกต้อง';
			if (formData.parent.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.parent.email)) {
				errors['parent.email'] = 'รูปแบบอีเมลไม่ถูกต้อง';
			}
			if (formData.parent.national_id && !/^\d{13}$/.test(formData.parent.national_id)) {
				errors['parent.national_id'] = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก';
			}
		}

		return Object.keys(errors).length === 0;
	}

	async function handleSubmit() {
		if (!validateForm()) {
			toast.error('กรุณากรอกข้อมูลให้ครบถ้วน');
			return;
		}

		loading = true;

		try {
			const { confirmPassword, ...payload } = formData;

			// Clean up payload - convert empty strings to undefined for optional fields
			const cleanedPayload = {
				...payload,
				email: payload.email || undefined,
				date_of_birth: payload.date_of_birth || undefined,
				grade_level: formData.grade_level || undefined,
				class_room: formData.class_room || undefined,
				student_number: undefined, // Force undefined to match API type (removing null)
				title: formData.title || undefined,
				parent: formData.parent_enabled
					? {
							...formData.parent,
							email: formData.parent.email || undefined,
							national_id: formData.parent.national_id || undefined
						}
					: undefined
			};

			const result = await createStudent(cleanedPayload);

			toast.success('เพิ่มนักเรียนสำเร็จ');
			goto(resolve(`/staff/students/${result.id}/edit`));
		} catch (error) {
			console.error('Failed to create student:', error);
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>เพิ่มนักเรียนใหม่ - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button href="/staff/students" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4" />
		</Button>
		<div>
			<h1 class="text-2xl font-bold text-foreground">เพิ่มนักเรียนใหม่</h1>
			<p class="text-sm text-muted-foreground">กรอกข้อมูลนักเรียนให้ครบถ้วน</p>
		</div>
	</div>

	<form
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
		class="space-y-6"
	>
		<!-- Student ID (First priority) -->
		<Card class="p-6">
			<div class="flex items-center gap-3 mb-6">
				<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
					<GraduationCap class="w-5 h-5 text-primary" />
				</div>
				<h2 class="text-xl font-semibold">ข้อมูลการศึกษา</h2>
			</div>

			<div class="space-y-4">
				<div>
					<Label for="student_id">
						รหัสนักเรียน <span class="text-destructive">*</span>
					</Label>
					<Input
						id="student_id"
						type="text"
						bind:value={formData.student_id}
						placeholder="66001"
						class={errors.student_id ? 'border-destructive font-mono text-lg' : 'font-mono text-lg'}
						disabled={loading}
						required
					/>
					{#if errors.student_id}
						<p class="text-xs text-destructive mt-1">{errors.student_id}</p>
					{/if}
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div>
						<Label>ระดับชั้น <span class="text-destructive">*</span></Label>
						<Select.Root type="single" value={selectedGradeId} onValueChange={handleGradeChange}>
							<Select.Trigger>
								{gradeLevels.find((g) => g.id === selectedGradeId)?.name || 'เลือกระดับชั้น'}
							</Select.Trigger>
							<Select.Content>
								{#each gradeLevels as gl}
									<Select.Item value={gl.id}>{gl.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						{#if errors.grade_level}
							<p class="text-sm text-destructive">{errors.grade_level}</p>
						{/if}
					</div>

					<!-- Classroom -->
					<div class="space-y-2">
						<Label>ห้องเรียน <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={formData.class_room} disabled={!selectedGradeId}>
							<Select.Trigger>
								{classrooms.find((c) => c.name === formData.class_room)?.name || 'เลือกห้องเรียน'}
							</Select.Trigger>
							<Select.Content>
								{#each filteredClassrooms as room}
									<Select.Item value={room.name}>{room.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</div>
			</div>
		</Card>

		<!-- Login Credentials -->
		<Card class="p-6">
			<div class="flex items-center gap-3 mb-6">
				<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
					<User class="w-5 h-5 text-primary" />
				</div>
				<h2 class="text-xl font-semibold">บัญชีผู้ใช้งาน</h2>
			</div>

			<div class="space-y-4">
				<div class="grid grid-cols-2 gap-4">
					<div>
						<Label for="password">
							รหัสผ่าน <span class="text-destructive">*</span>
						</Label>
						<Input
							id="password"
							type="password"
							bind:value={formData.password}
							placeholder="••••••••"
							class={errors.password ? 'border-destructive' : ''}
							disabled={loading}
							required
						/>
						{#if errors.password}
							<p class="text-xs text-destructive mt-1">{errors.password}</p>
						{/if}
					</div>

					<div>
						<Label for="confirmPassword">
							ยืนยันรหัสผ่าน <span class="text-destructive">*</span>
						</Label>
						<Input
							id="confirmPassword"
							type="password"
							bind:value={formData.confirmPassword}
							placeholder="••••••••"
							class={errors.confirmPassword ? 'border-destructive' : ''}
							disabled={loading}
							required
						/>
						{#if errors.confirmPassword}
							<p class="text-xs text-destructive mt-1">{errors.confirmPassword}</p>
						{/if}
					</div>
				</div>
			</div>
		</Card>

		<!-- Personal Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลส่วนตัว</h2>

			<div class="space-y-4">
				<div>
					<Label for="national_id">เลขบัตรประชาชน</Label>
					<Input
						id="national_id"
						type="text"
						bind:value={formData.national_id}
						placeholder="1234567890123 (ไม่บังคับ)"
						maxlength={13}
						class={errors.national_id ? 'border-destructive' : ''}
						disabled={loading}
					/>
					{#if errors.national_id}
						<p class="text-xs text-destructive mt-1">{errors.national_id}</p>
					{:else}
						<p class="text-xs text-muted-foreground mt-1">
							เลขบัตรประชาชนสำหรับใช้ในระบบตรวจสอบสิทธิ์อื่นๆ (ถ้ามี)
						</p>
					{/if}
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div>
						<Label for="title">คำนำหน้า</Label>
						<Select.Root type="single" bind:value={formData.title}>
							<Select.Trigger>{formData.title || 'เลือกคำนำหน้า'}</Select.Trigger>
							<Select.Content>
								<Select.Item value="เด็กชาย">เด็กชาย</Select.Item>
								<Select.Item value="เด็กหญิง">เด็กหญิง</Select.Item>
								<Select.Item value="นาย">นาย</Select.Item>
								<Select.Item value="นางสาว">นางสาว</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					<div>
						<Label for="gender">เพศ</Label>
						<Select.Root type="single" bind:value={formData.gender}>
							<Select.Trigger>
								{formData.gender === 'male'
									? 'ชาย'
									: formData.gender === 'female'
										? 'หญิง'
										: 'เลือกเพศ'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="male">ชาย</Select.Item>
								<Select.Item value="female">หญิง</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div>
						<Label for="first_name">
							ชื่อ <span class="text-destructive">*</span>
						</Label>
						<Input
							id="first_name"
							type="text"
							bind:value={formData.first_name}
							placeholder="ชื่อ"
							class={errors.first_name ? 'border-destructive' : ''}
							disabled={loading}
							required
						/>
						{#if errors.first_name}
							<p class="text-xs text-destructive mt-1">{errors.first_name}</p>
						{/if}
					</div>

					<div>
						<Label for="last_name">
							นามสกุล <span class="text-destructive">*</span>
						</Label>
						<Input
							id="last_name"
							type="text"
							bind:value={formData.last_name}
							placeholder="นามสกุล"
							class={errors.last_name ? 'border-destructive' : ''}
							disabled={loading}
							required
						/>
						{#if errors.last_name}
							<p class="text-xs text-destructive mt-1">{errors.last_name}</p>
						{/if}
					</div>
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div>
						<Label for="date_of_birth">วันเกิด</Label>
						<DatePicker bind:value={formData.date_of_birth} placeholder="เลือกวันเกิด" />
					</div>

					<div>
						<Label for="email">อีเมล</Label>
						<Input
							id="email"
							type="email"
							bind:value={formData.email}
							placeholder="email@school.ac.th (ไม่บังคับ)"
							class={errors.email ? 'border-destructive' : ''}
							disabled={loading}
						/>
						{#if errors.email}
							<p class="text-xs text-destructive mt-1">{errors.email}</p>
						{/if}
					</div>
				</div>
			</div>
		</Card>

		<!-- Parent Account -->
		<Card class="p-6">
			<div class="flex items-center gap-3 mb-6">
				<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
					<User class="w-5 h-5 text-primary" />
				</div>
				<div class="flex-1">
					<h2 class="text-xl font-semibold">บัญชีผู้ปกครอง</h2>
					<p class="text-sm text-muted-foreground">
						สร้างบัญชีผู้ใช้งานสำหรับผู้ปกครองพร้อมกับนักเรียน
					</p>
				</div>
				<div class="flex items-center gap-2">
					<Label for="parent_enabled" class="cursor-pointer">สร้างบัญชีผู้ปกครอง</Label>
					<Switch id="parent_enabled" bind:checked={formData.parent_enabled} />
				</div>
			</div>

			{#if formData.parent_enabled}
				<div class="space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div>
							<Label for="parent_relationship">ความสัมพันธ์</Label>
							<Select.Root type="single" bind:value={formData.parent.relationship}>
								<Select.Trigger
									>{formData.parent.relationship || 'เลือกความสัมพันธ์'}</Select.Trigger
								>
								<Select.Content>
									<Select.Item value="บิดา">บิดา</Select.Item>
									<Select.Item value="มารดา">มารดา</Select.Item>
									<Select.Item value="ผู้ปกครอง">ผู้ปกครอง</Select.Item>
								</Select.Content>
							</Select.Root>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<Label for="parent_first_name">
								ชื่อผู้ปกครอง <span class="text-destructive">*</span>
							</Label>
							<Input
								id="parent_first_name"
								type="text"
								bind:value={formData.parent.first_name}
								placeholder="ชื่อ"
								class={errors['parent.first_name'] ? 'border-destructive' : ''}
								disabled={loading}
								required
							/>
							{#if errors['parent.first_name']}
								<p class="text-xs text-destructive mt-1">{errors['parent.first_name']}</p>
							{/if}
						</div>

						<div>
							<Label for="parent_last_name">
								นามสกุลผู้ปกครอง <span class="text-destructive">*</span>
							</Label>
							<Input
								id="parent_last_name"
								type="text"
								bind:value={formData.parent.last_name}
								placeholder="นามสกุล"
								class={errors['parent.last_name'] ? 'border-destructive' : ''}
								disabled={loading}
								required
							/>
							{#if errors['parent.last_name']}
								<p class="text-xs text-destructive mt-1">{errors['parent.last_name']}</p>
							{/if}
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<Label for="parent_phone">
								เบอร์โทรศัพท์ <span class="text-destructive">*</span> (ใช้เป็น Username)
							</Label>
							<Input
								id="parent_phone"
								type="tel"
								bind:value={formData.parent.phone}
								placeholder="0812345678"
								maxlength={10}
								class={errors['parent.phone'] ? 'border-destructive' : ''}
								disabled={loading}
								required
							/>
							{#if errors['parent.phone']}
								<p class="text-xs text-destructive mt-1">{errors['parent.phone']}</p>
							{/if}
						</div>
						<div>
							<Label for="parent_email">อีเมล</Label>
							<Input
								id="parent_email"
								type="email"
								bind:value={formData.parent.email}
								placeholder="parent@example.com"
								class={errors['parent.email'] ? 'border-destructive' : ''}
								disabled={loading}
							/>
							{#if errors['parent.email']}
								<p class="text-xs text-destructive mt-1">{errors['parent.email']}</p>
							{/if}
						</div>
					</div>

					<div>
						<Label for="parent_national_id">เลขบัตรประชาชนผู้ปกครอง</Label>
						<Input
							id="parent_national_id"
							type="text"
							bind:value={formData.parent.national_id}
							placeholder="1234567890123"
							maxlength={13}
							class={errors['parent.national_id'] ? 'border-destructive' : ''}
							disabled={loading}
						/>
						{#if errors['parent.national_id']}
							<p class="text-xs text-destructive mt-1">{errors['parent.national_id']}</p>
						{/if}
					</div>
				</div>
			{/if}
		</Card>

		<!-- Actions -->
		<div class="flex gap-3">
			<Button type="submit" disabled={loading} class="flex-1">
				{#if loading}
					กำลังบันทึก...
				{:else}
					<Save class="w-4 h-4 mr-2" />
					บันทึก
				{/if}
			</Button>
			<Button type="button" variant="outline" href="/staff/students" disabled={loading}
				>ยกเลิก</Button
			>
		</div>
	</form>
</div>
