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
		parents: [] as {
			first_name: string;
			last_name: string;
			phone: string;
			relationship: string;
			national_id: string;
			email: string;
		}[]
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

		// Parents validation
		if (formData.parent_enabled && formData.parents.length === 0) {
			// If enabled but list is empty, add one default parent or show error?
			// Let's assume on enable we add one automatically
		}

		formData.parents.forEach((parent, index) => {
			if (!parent.first_name) errors[`parents.${index}.first_name`] = 'กรุณากรอกชื่อ';
			if (!parent.last_name) errors[`parents.${index}.last_name`] = 'กรุณากรอกนามสกุล';
			if (!parent.phone) errors[`parents.${index}.phone`] = 'กรุณากรอกเบอร์โทร';
			if (!/^\d{9,10}$/.test(parent.phone)) errors[`parents.${index}.phone`] = 'เบอร์โทรไม่ถูกต้อง';
			if (parent.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(parent.email)) {
				errors[`parents.${index}.email`] = 'รูปแบบอีเมลไม่ถูกต้อง';
			}
			if (parent.national_id && !/^\d{13}$/.test(parent.national_id)) {
				errors[`parents.${index}.national_id`] = 'เลขบัตรฯ ไม่ถูกต้อง';
			}
		});

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
				parents: formData.parent_enabled
					? formData.parents.map((p) => ({
							...p,
							email: p.email || undefined,
							national_id: p.national_id || undefined
						}))
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
					<Switch
						id="parent_enabled"
						checked={formData.parent_enabled}
						onCheckedChange={(v) => {
							formData.parent_enabled = v;
							if (v && formData.parents.length === 0) {
								formData.parents = [
									{
										first_name: '',
										last_name: '',
										phone: '',
										relationship: 'บิดา',
										national_id: '',
										email: ''
									}
								];
							}
						}}
					/>
				</div>
			</div>

			{#if formData.parent_enabled}
				<div class="space-y-6">
					{#each formData.parents as parent, i}
						<div class="p-4 border rounded-lg relative bg-muted/30">
							{#if i > 0}
								<Button
									variant="ghost"
									size="icon"
									class="absolute top-2 right-2 text-destructive hover:text-destructive hover:bg-destructive/10"
									onclick={() => {
										formData.parents = formData.parents.filter((_, index) => index !== i);
									}}
								>
									<span class="sr-only">ลบ</span>
									<svg
										xmlns="http://www.w3.org/2000/svg"
										width="16"
										height="16"
										viewBox="0 0 24 24"
										fill="none"
										stroke="currentColor"
										stroke-width="2"
										stroke-linecap="round"
										stroke-linejoin="round"
										class="lucide lucide-trash-2"
										><path d="M3 6h18" /><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" /><path
											d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
										/><line x1="10" x2="10" y1="11" y2="17" /><line
											x1="14"
											x2="14"
											y1="11"
											y2="17"
										/></svg
									>
								</Button>
							{/if}

							<h3 class="text-sm font-medium mb-4 flex items-center gap-2">
								<span
									class="flex items-center justify-center w-6 h-6 rounded-full bg-primary text-primary-foreground text-xs"
								>
									{i + 1}
								</span>
								ผู้ปกครองคนที่ {i + 1}
								{#if i === 0}<span class="text-xs text-muted-foreground">(ผู้ปกครองหลัก)</span>{/if}
							</h3>

							<div class="space-y-4">
								<div class="grid grid-cols-2 gap-4">
									<div>
										<Label>ความสัมพันธ์</Label>
										<Select.Root type="single" bind:value={parent.relationship}>
											<Select.Trigger>{parent.relationship || 'เลือกความสัมพันธ์'}</Select.Trigger>
											<Select.Content>
												<Select.Item value="บิดา">บิดา</Select.Item>
												<Select.Item value="มารดา">มารดา</Select.Item>
												<Select.Item value="ผู้ปกครอง">ผู้ปกครอง</Select.Item>
												<Select.Item value="ญาติ">ญาติ</Select.Item>
											</Select.Content>
										</Select.Root>
									</div>
								</div>

								<div class="grid grid-cols-2 gap-4">
									<div>
										<Label>
											ชื่อ <span class="text-destructive">*</span>
										</Label>
										<Input
											type="text"
											bind:value={parent.first_name}
											placeholder="ชื่อ"
											class={errors[`parents.${i}.first_name`] ? 'border-destructive' : ''}
											disabled={loading}
											required
										/>
										{#if errors[`parents.${i}.first_name`]}
											<p class="text-xs text-destructive mt-1">
												{errors[`parents.${i}.first_name`]}
											</p>
										{/if}
									</div>

									<div>
										<Label>
											นามสกุล <span class="text-destructive">*</span>
										</Label>
										<Input
											type="text"
											bind:value={parent.last_name}
											placeholder="นามสกุล"
											class={errors[`parents.${i}.last_name`] ? 'border-destructive' : ''}
											disabled={loading}
											required
										/>
										{#if errors[`parents.${i}.last_name`]}
											<p class="text-xs text-destructive mt-1">
												{errors[`parents.${i}.last_name`]}
											</p>
										{/if}
									</div>
								</div>

								<div class="grid grid-cols-2 gap-4">
									<div>
										<Label>
											เบอร์โทรศัพท์ <span class="text-destructive">*</span> (Username)
										</Label>
										<Input
											type="tel"
											bind:value={parent.phone}
											placeholder="0812345678"
											maxlength={10}
											class={errors[`parents.${i}.phone`] ? 'border-destructive' : ''}
											disabled={loading}
											required
										/>
										{#if errors[`parents.${i}.phone`]}
											<p class="text-xs text-destructive mt-1">{errors[`parents.${i}.phone`]}</p>
										{/if}
									</div>
									<div>
										<Label>อีเมล</Label>
										<Input
											type="email"
											bind:value={parent.email}
											placeholder="parent@example.com"
											class={errors[`parents.${i}.email`] ? 'border-destructive' : ''}
											disabled={loading}
										/>
										{#if errors[`parents.${i}.email`]}
											<p class="text-xs text-destructive mt-1">{errors[`parents.${i}.email`]}</p>
										{/if}
									</div>
								</div>

								<div>
									<Label>เลขบัตรประชาชน</Label>
									<Input
										type="text"
										bind:value={parent.national_id}
										placeholder="13 หลัก"
										maxlength={13}
										class={errors[`parents.${i}.national_id`] ? 'border-destructive' : ''}
										disabled={loading}
									/>
									{#if errors[`parents.${i}.national_id`]}
										<p class="text-xs text-destructive mt-1">
											{errors[`parents.${i}.national_id`]}
										</p>
									{/if}
								</div>
							</div>
						</div>
					{/each}

					<Button
						type="button"
						variant="outline"
						class="w-full border-dashed"
						onclick={() => {
							formData.parents = [
								...formData.parents,
								{
									first_name: '',
									last_name: '',
									phone: '',
									relationship: 'ผู้ปกครอง',
									national_id: '',
									email: ''
								}
							];
						}}
					>
						+ เพิ่มผู้ปกครองอีกท่าน
					</Button>
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
