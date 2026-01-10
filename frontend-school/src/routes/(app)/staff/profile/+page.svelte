<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '$lib/stores/auth';
	import { authAPI, type ProfileResponse } from '$lib/api/auth';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Select from '$lib/components/ui/select';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Avatar } from '$lib/components/ui/avatar';
	import ProfileImageUpload from '$lib/components/forms/ProfileImageUpload.svelte';
	import {
		ArrowLeft,
		Save,
		User,
		LoaderCircle,
		Calendar,
		Mail,
		Phone,
		MapPin,
		Shield,
		Lock
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	const user = $derived($authStore.user);

	// Full profile data from API
	let profile = $state<ProfileResponse | null>(null);

	// Form data - สำหรับฟิลด์ที่แก้ไขได้
	let formData = $state({
		// Editable fields
		title: '',
		nickname: '',
		email: '',
		phone: '',
		emergency_contact: '',
		line_id: '',
		date_of_birth: '',
		gender: 'male',
		address: '',
		profile_image_url: ''
	});

	// Read-only data - ข้อมูลที่แสดงผลเฉยๆ แก้ไม่ได้
	let readOnlyData = $derived({
		id: profile?.id || user?.id || '',
		national_id: profile?.nationalId || user?.nationalId || '',
		first_name: profile?.firstName || user?.firstName || '',
		last_name: profile?.lastName || user?.lastName || '',
		user_type: profile?.userType || user?.role || '',
		status: profile?.status || user?.status || '',
		created_at: profile?.createdAt || user?.createdAt || '',
		updated_at: profile?.updatedAt || '',
		primary_role_name: profile?.primaryRoleName || user?.primaryRoleName || ''
	});

	let saving = $state(false);
	let loading = $state(false);

	onMount(async () => {
		// Load full user profile from API
		loading = true;
		try {
			profile = await authAPI.getFullProfile();

			// Populate form with profile data
			formData = {
				title: profile.title || '',
				nickname: profile.nickname || '',
				email: profile.email || '',
				phone: profile.phone || '',
				emergency_contact: profile.emergencyContact || '',
				line_id: profile.lineId || '',
				date_of_birth: profile.dateOfBirth || '',
				gender: profile.gender || 'male',
				address: profile.address || '',
				profile_image_url: profile.profileImageUrl || ''
			};
		} catch (error) {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
			console.error('Failed to load profile:', error);
		} finally {
			loading = false;
		}
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();

		// Validation
		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			toast.error('รูปแบบอีเมลไม่ถูกต้อง');
			return;
		}

		saving = true;

		try {
			// Update profile via API
			const updatedProfile = await authAPI.updateProfile({
				title: formData.title || undefined,
				nickname: formData.nickname || undefined,
				email: formData.email || undefined,
				phone: formData.phone || undefined,
				emergencyContact: formData.emergency_contact || undefined,
				lineId: formData.line_id || undefined,
				dateOfBirth: formData.date_of_birth || undefined,
				gender: formData.gender || undefined,
				address: formData.address || undefined,
				profileImageUrl: formData.profile_image_url || undefined
			});

			// Update local profile state
			profile = updatedProfile;
			toast.success('บันทึกข้อมูลสำเร็จ');
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'ไม่สามารถบันทึกข้อมูลได้';
			toast.error(errorMessage);
			console.error('Failed to update profile:', error);
		} finally {
			saving = false;
		}
	}

	// Helper functions
	function getTitleLabel(value: string): string {
		const labels: Record<string, string> = {
			นาย: 'นาย',
			นาง: 'นาง',
			นางสาว: 'นางสาว',
			'ดร.': 'ดร.',
			'ศ.': 'ศ.',
			'รศ.': 'รศ.',
			'ผศ.': 'ผศ.',
			'ศ.ดร.': 'ศ.ดร.',
			'รศ.ดร.': 'รศ.ดร.',
			'ผศ.ดร.': 'ผศ.ดร.'
		};
		return labels[value] || 'เลือกคำนำหน้า';
	}

	function getGenderLabel(value: string): string {
		const labels: Record<string, string> = {
			male: 'ชาย',
			female: 'หญิง',
			other: 'อื่นๆ'
		};
		return labels[value] || 'เลือกเพศ';
	}

	function getStatusLabel(status: string): string {
		const labels: Record<string, string> = {
			active: 'ใช้งาน',
			inactive: 'ไม่ใช้งาน',
			suspended: 'ระงับ',
			resigned: 'ลาออก',
			retired: 'เกษียณ'
		};
		return labels[status] || status;
	}
</script>

<svelte:head>
	<title>โปรไฟล์ของฉัน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button variant="ghost" size="icon" onclick={() => goto(resolve('/staff'))}>
				<ArrowLeft class="h-5 w-5" />
			</Button>
			<div>
				<h1 class="text-3xl font-bold text-foreground">โปรไฟล์ของฉัน</h1>
				<p class="text-muted-foreground mt-1">จัดการข้อมูลส่วนตัวของคุณ</p>
			</div>
		</div>
		<Button onclick={handleSubmit} disabled={saving || loading} class="gap-2">
			<Save class="h-4 w-4" />
			{saving ? 'กำลังบันทึก...' : 'บันทึกการเปลี่ยนแปลง'}
		</Button>
	</div>

	{#if loading}
		<div class="flex justify-center items-center py-20">
			<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
		</div>
	{:else}
		<form onsubmit={handleSubmit} class="space-y-6">
			<!-- Profile Avatar -->
			<Card>
				<CardHeader>
					<CardTitle>รูปโปรไฟล์</CardTitle>
					<CardDescription>อัพโหลดรูปโปรไฟล์ของคุณ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-6">
					<div class="flex flex-col md:flex-row items-center gap-6">
						<!-- Avatar Display -->
						<div class="flex-shrink-0">
							<Avatar
								src={formData.profile_image_url}
								initials={(readOnlyData.first_name?.charAt(0) || 'U') +
									(readOnlyData.last_name?.charAt(0) || '')}
								size="xl"
								shape="circle"
								class="ring-4 ring-background shadow-lg"
							/>
						</div>

						<!-- User Info & Upload -->
						<div class="flex-1 space-y-4">
							<div>
								<p class="font-semibold text-lg text-foreground">
									{readOnlyData.first_name}
									{readOnlyData.last_name}
								</p>
								<p class="text-sm text-muted-foreground">
									{readOnlyData.primary_role_name || 'ผู้ใช้งาน'}
								</p>
							</div>

							<!-- Upload Component -->
							<ProfileImageUpload
								currentImage={formData.profile_image_url}
								onsuccess={async ({ url }: { url: string; fileId: string }) => {
									formData.profile_image_url = url;
									try {
										await authAPI.updateProfile({ profileImageUrl: url });
										await authAPI.checkAuth(); // Refresh header avatar
										toast.success('อัปเดตรูปโปรไฟล์เรียบร้อยแล้ว');
									} catch (e) {
										console.error(e);
										toast.warning('อัปโหลดสำเร็จ แต่การบันทึกข้อมูลมีปัญหา');
									}
								}}
								onerror={(err: string) => toast.error(err)}
								maxSizeMB={5}
							/>
						</div>
					</div>
				</CardContent>
			</Card>

			<!-- Read-Only Information -->
			<Card>
				<CardHeader>
					<CardTitle class="flex items-center gap-2">
						<Lock class="w-5 h-5" />
						ข้อมูลระบบ (ไม่สามารถแก้ไขได้)
					</CardTitle>
					<CardDescription>ข้อมูลเหล่านี้ไม่สามารถแก้ไขได้ กรุณาติดต่อผู้ดูแลระบบ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-4">
					<div class="grid gap-4 md:grid-cols-2">
						<!-- National ID -->
						<div class="space-y-2">
							<Label>เลขบัตรประชาชน</Label>
							<Input value={readOnlyData.national_id || 'ไม่ระบุ'} disabled class="bg-muted" />
						</div>

						<!-- User ID -->
						<div class="space-y-2">
							<Label>User ID</Label>
							<Input value={readOnlyData.id} disabled class="bg-muted font-mono text-xs" />
						</div>
					</div>

					<div class="grid gap-4 md:grid-cols-2">
						<!-- First Name -->
						<div class="space-y-2">
							<Label>ชื่อ</Label>
							<Input value={readOnlyData.first_name} disabled class="bg-muted" />
						</div>

						<!-- Last Name -->
						<div class="space-y-2">
							<Label>นามสกุล</Label>
							<Input value={readOnlyData.last_name} disabled class="bg-muted" />
						</div>
					</div>

					<div class="grid gap-4 md:grid-cols-3">
						<!-- User Type -->
						<div class="space-y-2">
							<Label>ประเภทผู้ใช้</Label>
							<Input value={readOnlyData.user_type} disabled class="bg-muted capitalize" />
						</div>

						<!-- Status -->
						<div class="space-y-2">
							<Label>สถานะ</Label>
							<Input value={getStatusLabel(readOnlyData.status)} disabled class="bg-muted" />
						</div>

						<!-- Created At -->
						<div class="space-y-2">
							<Label>วันที่สร้างบัญชี</Label>
							<Input
								value={readOnlyData.created_at
									? new Date(readOnlyData.created_at).toLocaleDateString('th-TH')
									: 'ไม่ระบุ'}
								disabled
								class="bg-muted"
							/>
						</div>
					</div>

					<!-- Primary Role -->
					<div class="space-y-2">
						<Label class="flex items-center gap-2">
							<Shield class="w-4 h-4" />
							บทบาทหลัก
						</Label>
						<Input value={readOnlyData.primary_role_name || 'ไม่ระบุ'} disabled class="bg-muted" />
						<p class="text-xs text-muted-foreground">ต้องการเปลี่ยนบทบาท กรุณาติดต่อผู้ดูแลระบบ</p>
					</div>
				</CardContent>
			</Card>

			<!-- Editable Personal Information -->
			<Card>
				<CardHeader>
					<CardTitle class="flex items-center gap-2">
						<User class="w-5 h-5" />
						ข้อมูลส่วนตัว
					</CardTitle>
					<CardDescription>อัพเดทข้อมูลส่วนตัวของคุณ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-4">
					<!-- Title and Gender -->
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="title">คำนำหน้า</Label>
							<Select.Root type="single" bind:value={formData.title}>
								<Select.Trigger>{getTitleLabel(formData.title)}</Select.Trigger>
								<Select.Content>
									<Select.Item value="">ไม่ระบุ</Select.Item>
									<Select.Item value="นาย">นาย</Select.Item>
									<Select.Item value="นาง">นาง</Select.Item>
									<Select.Item value="นางสาว">นางสาว</Select.Item>
									<Select.Item value="ดร.">ดร.</Select.Item>
									<Select.Item value="ศ.">ศ.</Select.Item>
									<Select.Item value="รศ.">รศ.</Select.Item>
									<Select.Item value="ผศ.">ผศ.</Select.Item>
									<Select.Item value="ศ.ดร.">ศ.ดร.</Select.Item>
									<Select.Item value="รศ.ดร.">รศ.ดร.</Select.Item>
									<Select.Item value="ผศ.ดร.">ผศ.ดร.</Select.Item>
								</Select.Content>
							</Select.Root>
						</div>

						<div class="space-y-2">
							<Label for="gender">เพศ</Label>
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

					<!-- Nickname -->
					<div class="space-y-2">
						<Label for="nickname">ชื่อเล่น</Label>
						<Input id="nickname" bind:value={formData.nickname} placeholder="ชื่อเล่น" />
					</div>

					<!-- Date of Birth -->
					<div class="space-y-2">
						<Label for="dateOfBirth">
							<Calendar class="w-4 h-4 inline mr-1" />
							วันเกิด
						</Label>
						<DatePicker bind:value={formData.date_of_birth} placeholder="เลือกวันเกิด" />
					</div>
				</CardContent>
			</Card>

			<!-- Contact Information -->
			<Card>
				<CardHeader>
					<CardTitle class="flex items-center gap-2">
						<Phone class="w-5 h-5" />
						ข้อมูลการติดต่อ
					</CardTitle>
					<CardDescription>ข้อมูลสำหรับการติดต่อ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-4">
					<!-- Email -->
					<div class="space-y-2">
						<Label for="email">
							<Mail class="w-4 h-4 inline mr-1" />
							อีเมล
						</Label>
						<Input
							id="email"
							type="email"
							bind:value={formData.email}
							placeholder="example@school.ac.th"
						/>
					</div>

					<!-- Phone Numbers -->
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="phone">
								<Phone class="w-4 h-4 inline mr-1" />
								หมายเลขโทรศัพท์
							</Label>
							<Input id="phone" type="tel" bind:value={formData.phone} placeholder="081-234-5678" />
						</div>

						<div class="space-y-2">
							<Label for="emergencyContact">
								<Phone class="w-4 h-4 inline mr-1" />
								เบอร์ติดต่อฉุกเฉิน
							</Label>
							<Input
								id="emergencyContact"
								type="tel"
								bind:value={formData.emergency_contact}
								placeholder="081-234-5678"
							/>
						</div>
					</div>

					<!-- Line ID -->
					<div class="space-y-2">
						<Label for="lineId">Line ID</Label>
						<Input id="lineId" bind:value={formData.line_id} placeholder="@lineid" />
					</div>

					<!-- Address -->
					<div class="space-y-2">
						<Label for="address">
							<MapPin class="w-4 h-4 inline mr-1" />
							ที่อยู่
						</Label>
						<Textarea
							id="address"
							bind:value={formData.address}
							placeholder="ที่อยู่ปัจจุบัน"
							rows={3}
						/>
					</div>
				</CardContent>
			</Card>
		</form>
	{/if}
</div>
