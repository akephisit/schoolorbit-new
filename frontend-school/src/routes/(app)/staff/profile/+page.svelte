<script lang="ts">
	import StaffCareerHistory from '#lib/components/staff/StaffCareerHistory.svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { authStore } from '#lib/stores/auth.js';
	import { authAPI, type ProfileResponse } from '#lib/api/auth.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import ProfileImageUpload from '#lib/components/forms/ProfileImageUpload.svelte';
	import { Save, User, Calendar, Mail, Phone, MapPin, Shield, Lock } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let { data }: PageProps = $props();
	const profileRequest = new LatestRequest();
	let profileEpoch = 0;
	let profileActive = false;
	let profileOwner = '';
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
		profile_image_file_id: ''
	});

	// Read-only data - ข้อมูลที่แสดงผลเฉยๆ แก้ไม่ได้
	let readOnlyData = $derived({
		id: profile?.id || user?.id || '',
		username: profile?.username || user?.username || '',
		national_id: profile?.nationalId || '',
		first_name: profile?.firstName || user?.firstName || '',
		last_name: profile?.lastName || user?.lastName || '',
		user_type: profile?.userType || user?.role || '',
		status: profile?.status || user?.status || '',
		created_at: profile?.createdAt || '',
		updated_at: profile?.updatedAt || '',
		primary_role_name: profile?.primaryRoleName || user?.primaryRoleName || ''
	});

	let saving = $state(false);
	let loading = $state(true);
	let profileLoadError = $state('');

	function applyLoadedProfile(loaded: ProfileResponse) {
		profile = loaded;
		formData = {
			title: loaded.title ?? '',
			nickname: loaded.nickname ?? '',
			email: loaded.email ?? '',
			phone: loaded.phone ?? '',
			emergency_contact: loaded.emergencyContact ?? '',
			line_id: loaded.lineId ?? '',
			date_of_birth: loaded.dateOfBirth ?? '',
			gender: loaded.gender ?? 'male',
			address: loaded.address ?? '',
			profile_image_file_id: loaded.profileImageFileId ?? ''
		};
	}
	$effect.pre(() => {
		const source = data.profile;
		const owner = $authStore.user?.id ?? '';
		untrack(() => {
			profileActive = true;
			if (profileOwner !== owner) {
				profileOwner = owner;
				profile = null;
				profileEpoch += 1;
				saving = false;
			}
			const ticket = profileRequest.begin();
			loading = true;
			profileLoadError = '';
			void source.then((result) => {
				if (!profileRequest.isCurrent(ticket.revision)) return;
				loading = false;
				if (result.ok && result.data?.id === profileOwner) applyLoadedProfile(result.data);
				else profileLoadError = result.ok ? 'คุณไม่มีสิทธิ์เข้าถึงโปรไฟล์นี้' : result.error;
			});
		});
		return () => {
			profileActive = false;
			profileEpoch += 1;
			profileRequest.abort();
		};
	});
	async function loadProfile() {
		const ticket = profileRequest.begin();
		loading = true;
		profileLoadError = '';
		const result = await captureRouteLoad(
			authAPI.getFullProfile({ signal: ticket.signal }),
			'โหลดโปรไฟล์ไม่สำเร็จ'
		);
		if (!profileRequest.isCurrent(ticket.revision)) return;
		loading = false;
		if (result.ok && result.data.id === profileOwner) applyLoadedProfile(result.data);
		else profileLoadError = result.ok ? 'คุณไม่มีสิทธิ์เข้าถึงโปรไฟล์นี้' : result.error;
	}
	async function saveProfileImage(fileId: string | null) {
		if (!profileActive || !profile) return;
		const sourceEpoch = profileEpoch;
		try {
			const updated = await authAPI.updateProfile({ profileImageFileId: fileId });
			if (sourceEpoch !== profileEpoch || !profileActive) return;
			profileRequest.abort();
			loading = false;
			profile = updated;
			formData.profile_image_file_id = updated.profileImageFileId ?? '';
			const result = await authAPI.refreshCurrentUser({ silent: true });
			if (sourceEpoch !== profileEpoch || !profileActive) return;
			if (result === 'authenticated') toast.success('อัปเดตรูปโปรไฟล์เรียบร้อยแล้ว');
			else if (result === 'unavailable')
				toast.warning('บันทึกรูปแล้ว แต่ยังรีเฟรชรูปบนแถบเมนูไม่ได้');
		} catch (error) {
			if (sourceEpoch === profileEpoch && profileActive)
				toast.error(error instanceof Error ? error.message : 'บันทึกรูปไม่สำเร็จ');
		}
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();

		// Validation
		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			toast.error('รูปแบบอีเมลไม่ถูกต้อง');
			return;
		}

		if (!profileActive || !profile) return;
		const sourceEpoch = profileEpoch;
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
				profileImageFileId: formData.profile_image_file_id || null
			});

			// Update local profile state
			if (sourceEpoch !== profileEpoch || !profileActive) return;
			profileRequest.abort();
			loading = false;
			profile = updatedProfile;
			toast.success('บันทึกข้อมูลสำเร็จ');
		} catch (error) {
			if (sourceEpoch !== profileEpoch || !profileActive) return;
			const errorMessage = error instanceof Error ? error.message : 'ไม่สามารถบันทึกข้อมูลได้';
			toast.error(errorMessage);
		} finally {
			if (sourceEpoch === profileEpoch && profileActive) saving = false;
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

<PageShell title="โปรไฟล์ของฉัน" description="จัดการข้อมูลส่วนตัวของคุณ" backHref="/staff">
	{#snippet actions()}
		<Button onclick={handleSubmit} disabled={saving || loading} class="gap-2">
			<Save class="h-4 w-4" />
			{saving ? 'กำลังบันทึก...' : 'บันทึกการเปลี่ยนแปลง'}
		</Button>
	{/snippet}
	<div data-testid="staff-own-profile" aria-busy={loading}>
		{#if loading && !profile}
			<div role="status" aria-label="กำลังโหลดโปรไฟล์">
				<PageSkeleton variant="form" rows={6} />
			</div>
		{:else if profileLoadError && !profile}
			<PageState
				variant="error"
				title="โหลดโปรไฟล์ไม่สำเร็จ"
				description={profileLoadError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadProfile}
			/>
		{:else}
			<form onsubmit={handleSubmit} class="space-y-6">
				<!-- Profile Avatar -->
				<Card>
					<CardHeader>
						<CardTitle>รูปโปรไฟล์</CardTitle>
						<CardDescription>อัพโหลดรูปโปรไฟล์ของคุณ</CardDescription>
					</CardHeader>
					<CardContent class="space-y-6">
						<div class="flex flex-col md:flex-row items-center md:items-start gap-8">
							<!-- Image Upload Component (Shows Avatar + Edit Button) -->
							<div class="flex-shrink-0">
								<ProfileImageUpload
									currentFileId={formData.profile_image_file_id}
									resourceId={profile?.id}
									onsuccess={({ fileId }: { fileId: string | null }) =>
										void saveProfileImage(fileId)}
									onerror={(err: string) => toast.error(err)}
								/>
							</div>

							<!-- User Info -->
							<div class="flex-1 space-y-2 text-center md:text-left py-4">
								<div>
									<h3 class="font-bold text-2xl text-foreground tracking-tight">
										{readOnlyData.first_name}
										{readOnlyData.last_name}
									</h3>
									<p class="text-muted-foreground font-medium">
										{readOnlyData.primary_role_name || 'ผู้ใช้งาน'}
									</p>
								</div>

								<div class="pt-4 text-sm text-muted-foreground">
									<p>อัปโหลดรูปภาพของคุณเพื่อใช้เป็นรูปโปรไฟล์</p>
									<p>รองรับไฟล์ JPG, PNG, WebP ขนาดไม่เกิน 5 MB</p>
								</div>
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
						<CardDescription>ข้อมูลเหล่านี้ไม่สามารถแก้ไขได้ กรุณาติดต่อผู้ดูแลระบบ</CardDescription
						>
					</CardHeader>
					<CardContent class="space-y-4">
						<div class="grid gap-4 md:grid-cols-2">
							<!-- Username -->
							<div class="space-y-2">
								<Label>ชื่อผู้ใช้งาน (Username)</Label>
								<Input value={readOnlyData.username || 'ไม่ระบุ'} disabled class="bg-muted" />
							</div>

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
							<Input
								value={readOnlyData.primary_role_name || 'ไม่ระบุ'}
								disabled
								class="bg-muted"
							/>
							<p class="text-xs text-muted-foreground">
								ต้องการเปลี่ยนบทบาท กรุณาติดต่อผู้ดูแลระบบ
							</p>
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
								<Input
									id="phone"
									type="tel"
									bind:value={formData.phone}
									placeholder="081-234-5678"
								/>
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
	{#if user?.id && $can.hasAny(PERMISSIONS.STAFF_PROFILE_READ_OWN, PERMISSIONS.STAFF_PROFILE_READ_SCHOOL, PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT, PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE)}<StaffCareerHistory
			staffId={user.id}
			initial={data.careerHistory}
			canEdit={$can.has(PERMISSIONS.STAFF_UPDATE_ALL)}
			onCurrentChanged={() => Promise.resolve()}
		/>{/if}
</PageShell>
