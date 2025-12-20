<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import {
		getStaffProfile,
		updateStaff,
		listRoles,
		listDepartments,
		type Role,
		type Department,
		type StaffProfileResponse
	} from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { User, ArrowLeft, Loader2, Save } from 'lucide-svelte';
	import { onMount } from 'svelte';

	const staffId = $derived($page.params.id);

	// Loading states
	let loading = $state(false);
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
		emergency_contact: '',
		line_id: '',
		date_of_birth: '',
		gender: 'male',
		address: '',
		status: 'active'
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
					title: staff.title || '',
					first_name: staff.first_name,
					last_name: staff.last_name,
					nickname: staff.nickname || '',
					phone: staff.phone || '',
					emergency_contact: '',
					line_id: '',
					date_of_birth: '',
					gender: 'male',
					address: '',
					status: staff.status
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

		saving = true;
		errors = {};
		successMessage = '';

		try {
			const result = await updateStaff(staffId, formData);

			if (result.success) {
				successMessage = 'บันทึกข้อมูลสำเร็จ';
				setTimeout(() => {
					goto(`/staff/${staffId}`);
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
				<Loader2 class="w-8 h-8 animate-spin text-muted-foreground mx-auto mb-4" />
				<p class="text-muted-foreground">กำลังโหลดข้อมูล...</p>
			</div>
		{:else if errors.load}
			<div class="bg-destructive/10 border border-destructive/20 rounded-lg p-6 text-center">
				<p class="text-destructive">{errors.load}</p>
				<Button onclick={loadStaffProfile} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
			</div>
		{:else if staff}
			<form onsubmit|preventDefault={handleSubmit}>
				<div class="bg-card border border-border rounded-lg p-6">
					<h2 class="text-xl font-semibold mb-6">ข้อมูลส่วนตัว</h2>

					<div class="space-y-4">
						<div class="grid grid-cols-2 gap-4">
							<div>
								<label class="block text-sm font-medium mb-2">
									คำนำหน้า <span class="text-destructive">*</span>
								</label>
								<select
									bind:value={formData.title}
									class="w-full px-3 py-2 border border-border rounded-md"
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
								<label class="block text-sm font-medium mb-2">สถานะ</label>
								<select
									bind:value={formData.status}
									class="w-full px-3 py-2 border border-border rounded-md"
								>
									<option value="active">ใช้งาน</option>
									<option value="inactive">ไม่ใช้งาน</option>
									<option value="suspended">ระงับ</option>
									<option value="resigned">ลาออก</option>
									<option value="retired">เกษียณ</option>
								</select>
							</div>
						</div>

						<div class="grid grid-cols-2 gap-4">
							<div>
								<label class="block text-sm font-medium mb-2">
									ชื่อ <span class="text-destructive">*</span>
								</label>
								<input
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
								<label class="block text-sm font-medium mb-2">
									นามสกุล <span class="text-destructive">*</span>
								</label>
								<input
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
							<label class="block text-sm font-medium mb-2">ชื่อเล่น</label>
							<input
								type="text"
								bind:value={formData.nickname}
								placeholder="ชื่อเล่น"
								class="w-full px-3 py-2 border border-border rounded-md"
							/>
						</div>

						<div>
							<label class="block text-sm font-medium mb-2">หมายเลขโทรศัพท์</label>
							<input
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

						<div class="bg-muted/50 p-4 rounded-lg">
							<p class="text-sm text-muted-foreground">
								<strong>หมายเหตุ:</strong> การแก้ไขบทบาท, ฝ่าย และวิชาที่สอน กรุณาติดต่อผู้ดูแลระบบ
							</p>
						</div>
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
							<Loader2 class="w-4 h-4 mr-2 animate-spin" />
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
