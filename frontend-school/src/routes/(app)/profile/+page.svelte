<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Select from '$lib/components/ui/select';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { ArrowLeft, Save, User, LoaderCircle, Calendar, Mail, Phone, MapPin } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	const user = $derived($authStore.user);

	// Form data
	let formData = $state({
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
		address: ''
	});

	let saving = $state(false);
	let loading = $state(false);

	onMount(() => {
		if (user) {
			// Populate form with user data
			formData = {
				title: 'นาย', // TODO: Get from API
				first_name: user.firstName || '',
				last_name: user.lastName || '',
				nickname: '',
				email: user.email || '',
				phone: user.phone || '',
				emergency_contact: '',
				line_id: '',
				date_of_birth: '',
				gender: 'male',
				address: ''
			};
		}
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();

		// Validation
		if (!formData.first_name || !formData.last_name) {
			toast.error('กรุณากรอกชื่อและนามสกุล');
			return;
		}

		if (formData.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(formData.email)) {
			toast.error('รูปแบบอีเมลไม่ถูกต้อง');
			return;
		}

		saving = true;

		try {
			// TODO: Implement API call to update profile
			await new Promise((resolve) => setTimeout(resolve, 1000)); // Simulated API call
			toast.success('บันทึกข้อมูลสำเร็จ');
		} catch (error) {
			toast.error('ไม่สามารถบันทึกข้อมูลได้');
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
			'ผศ.': 'ผศ.'
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
</script>

<svelte:head>
	<title>โปรไฟล์ของฉัน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button variant="ghost" size="icon" onclick={() => goto(resolve('/dashboard'))}>
				<ArrowLeft class="h-5 w-5" />
			</Button>
			<div>
				<h1 class="text-3xl font-bold text-foreground">โปรไฟล์ของฉัน</h1>
				<p class="text-muted-foreground mt-1">จัดการข้อมูลส่วนตัวของคุณ</p>
			</div>
		</div>
		<Button onclick={handleSubmit} disabled={saving} class="gap-2">
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
				<CardContent class="flex items-center gap-6">
					<div
						class="w-24 h-24 rounded-full bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center shadow-lg ring-4 ring-background flex-shrink-0"
					>
						<span class="text-3xl font-bold text-primary-foreground">
							{user?.firstName?.charAt(0) || 'U'}{user?.lastName?.charAt(0) || ''}
						</span>
					</div>
					<div class="flex-1">
						<p class="font-semibold text-lg text-foreground mb-1">
							{user?.firstName || ''}
							{user?.lastName || ''}
						</p>
						<p class="text-sm text-muted-foreground mb-3">{user?.primaryRoleName || 'ผู้ใช้งาน'}</p>
						<Button variant="outline" size="sm" type="button">
							<User class="h-4 w-4 mr-2" />
							เปลี่ยนรูปโปรไฟล์
						</Button>
					</div>
				</CardContent>
			</Card>

			<!-- Personal Information -->
			<Card>
				<CardHeader>
					<CardTitle class="flex items-center gap-2">
						<User class="w-5 h-5" />
						ข้อมูลส่วนตัว
					</CardTitle>
					<CardDescription>อัพเดทข้อมูลประวัติส่วนตัวของคุณ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-4">
					<!-- Title and Gender -->
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="title">คำนำหน้า *</Label>
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

						<div class="space-y-2">
							<Label for="gender">เพศ *</Label>
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

					<!-- Name -->
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="firstName">ชื่อ *</Label>
							<Input id="firstName" bind:value={formData.first_name} placeholder="ชื่อ" required />
						</div>

						<div class="space-y-2">
							<Label for="lastName">นามสกุล *</Label>
							<Input id="lastName" bind:value={formData.last_name} placeholder="นามสกุล" required />
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

			<!-- Account Information -->
			<Card>
				<CardHeader>
					<CardTitle>ข้อมูลบัญชี</CardTitle>
					<CardDescription>ข้อมูลบัญชีและสถานะการใช้งาน</CardDescription>
				</CardHeader>
				<CardContent>
					<div class="grid gap-4 md:grid-cols-3">
						<div class="space-y-1">
							<p class="text-sm font-medium text-muted-foreground">บทบาท</p>
							<p class="text-base text-foreground">{user?.primaryRoleName || 'ไม่ระบุ'}</p>
						</div>
						<div class="space-y-1">
							<p class="text-sm font-medium text-muted-foreground">สถานะ</p>
							<p class="text-base text-foreground">
								{user?.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
							</p>
						</div>
						<div class="space-y-1">
							<p class="text-sm font-medium text-muted-foreground">วันที่สร้างบัญชี</p>
							<p class="text-base text-foreground">
								{user?.createdAt
									? new Date(user.createdAt).toLocaleDateString('th-TH', {
											year: 'numeric',
											month: 'long',
											day: 'numeric'
										})
									: 'ไม่ระบุ'}
							</p>
						</div>
					</div>
				</CardContent>
			</Card>
		</form>
	{/if}
</div>
