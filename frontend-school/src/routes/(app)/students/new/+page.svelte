<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Card } from '$lib/components/ui/card';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, User, Save } from 'lucide-svelte';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { createStudent } from '$lib/api/students';

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
		gender: 'male'
	});

	let errors = $state<Record<string, string>>({});
	let loading = $state(false);

	function validateForm(): boolean {
		errors = {};

		// Required fields
		if (!formData.national_id) {
			errors.national_id = 'กรุณากรอกเลขบัตรประชาชน';
		} else if (!/^\d{13}$/.test(formData.national_id)) {
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

			const result = await createStudent({
				...payload,
				student_number: payload.student_number || undefined
			});

			toast.success('เพิ่มนักเรียนสำเร็จ');
			goto(resolve(`/students/${result.id}/edit`));
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

<div class="container mx-auto p-6 max-w-4xl space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button href="/students" variant="ghost" size="sm">
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
		<!-- Login Credentials -->
		<Card class="p-6">
			<div class="flex items-center gap-3 mb-6">
				<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
					<User class="w-5 h-5 text-primary" />
				</div>
				<h2 class="text-xl font-semibold">ข้อมูลสำหรับเข้าสู่ระบบ</h2>
			</div>

			<div class="space-y-4">
				<div>
					<Label for="national_id">
						เลขบัตรประชาชน <span class="text-destructive">*</span>
					</Label>
					<Input
						id="national_id"
						type="text"
						bind:value={formData.national_id}
						placeholder="1234567890123"
						maxlength={13}
						class={errors.national_id ? 'border-destructive' : ''}
						disabled={loading}
						required
					/>
					{#if errors.national_id}
						<p class="text-xs text-destructive mt-1">{errors.national_id}</p>
					{:else}
						<p class="text-xs text-muted-foreground mt-1">ใช้เลขบัตรประชาชนนี้ในการเข้าสู่ระบบ</p>
					{/if}
				</div>

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

		<!-- Student Information-->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลนักเรียน</h2>

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
						class={errors.student_id ? 'border-destructive' : ''}
						disabled={loading}
						required
					/>
					{#if errors.student_id}
						<p class="text-xs text-destructive mt-1">{errors.student_id}</p>
					{/if}
				</div>

				<div class="grid grid-cols-3 gap-4">
					<div>
						<Label for="grade_level">ระดับชั้น</Label>
						<Input
							id="grade_level"
							type="text"
							bind:value={formData.grade_level}
							placeholder="ม.1"
							disabled={loading}
						/>
					</div>

					<div>
						<Label for="class_room">ห้อง</Label>
						<Input
							id="class_room"
							type="text"
							bind:value={formData.class_room}
							placeholder="1"
							disabled={loading}
						/>
					</div>

					<div>
						<Label for="student_number">เลขที่</Label>
						<Input
							id="student_number"
							type="number"
							bind:value={formData.student_number}
							placeholder="1"
							disabled={loading}
						/>
					</div>
				</div>
			</div>
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
			<Button type="button" variant="outline" href="/students" disabled={loading}>ยกเลิก</Button>
		</div>
	</form>
</div>
