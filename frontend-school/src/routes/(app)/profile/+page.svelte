<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { User, Mail, Phone, Calendar, Save, ArrowLeft } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	const user = $derived($authStore.user);

	let formData = $state({
		firstName: '',
		lastName: '',
		email: '',
		phone: '',
		nationalId: ''
	});

	let saving = $state(false);

	onMount(() => {
		if (user) {
			formData = {
				firstName: user.firstName || '',
				lastName: user.lastName || '',
				email: user.email || '',
				phone: user.phone || '',
				nationalId: user.nationalId || ''
			};
		}
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();
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

	<div class="grid gap-6 lg:grid-cols-3">
		<!-- Profile Avatar -->
		<Card class="lg:col-span-1">
			<CardHeader>
				<CardTitle>รูปโปรไฟล์</CardTitle>
				<CardDescription>อัพโหลดรูปโปรไฟล์ของคุณ</CardDescription>
			</CardHeader>
			<CardContent class="flex flex-col items-center gap-4">
				<div
					class="w-32 h-32 rounded-full bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center shadow-lg ring-4 ring-background"
				>
					<span class="text-4xl font-bold text-primary-foreground">
						{user?.firstName?.charAt(0) || 'U'}{user?.lastName?.charAt(0) || ''}
					</span>
				</div>
				<div class="text-center">
					<p class="font-semibold text-foreground">
						{user?.firstName || ''}
						{user?.lastName || ''}
					</p>
					<p class="text-sm text-muted-foreground">{user?.primaryRoleName || 'ผู้ใช้งาน'}</p>
				</div>
				<Button variant="outline" size="sm" class="w-full">
					<User class="h-4 w-4 mr-2" />
					เปลี่ยนรูปโปรไฟล์
				</Button>
			</CardContent>
		</Card>

		<!-- Personal Information -->
		<Card class="lg:col-span-2">
			<CardHeader>
				<CardTitle>ข้อมูลส่วนตัว</CardTitle>
				<CardDescription>อัพเดทข้อมูลประวัติส่วนตัวของคุณ</CardDescription>
			</CardHeader>
			<CardContent>
				<form onsubmit={handleSubmit} class="space-y-4">
					<div class="grid gap-4 md:grid-cols-2">
						<!-- First Name -->
						<div class="space-y-2">
							<Label for="firstName">
								<User class="w-4 h-4 inline mr-1" />
								ชื่อ *
							</Label>
							<Input id="firstName" bind:value={formData.firstName} placeholder="ชื่อ" required />
						</div>

						<!-- Last Name -->
						<div class="space-y-2">
							<Label for="lastName">
								<User class="w-4 h-4 inline mr-1" />
								นามสกุล *
							</Label>
							<Input id="lastName" bind:value={formData.lastName} placeholder="นามสกุล" required />
						</div>
					</div>

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

					<!-- Phone -->
					<div class="space-y-2">
						<Label for="phone">
							<Phone class="w-4 h-4 inline mr-1" />
							เบอร์โทรศัพท์
						</Label>
						<Input id="phone" type="tel" bind:value={formData.phone} placeholder="0812345678" />
					</div>

					<!-- National ID -->
					<div class="space-y-2">
						<Label for="nationalId">
							<Calendar class="w-4 h-4 inline mr-1" />
							เลขบัตรประชาชน
						</Label>
						<Input
							id="nationalId"
							bind:value={formData.nationalId}
							placeholder="1234567890123"
							maxlength={13}
							disabled
							class="bg-muted"
						/>
						<p class="text-xs text-muted-foreground">
							ไม่สามารถแก้ไขเลขบัตรประชาชนได้ กรุณาติดต่อผู้ดูแลระบบ
						</p>
					</div>
				</form>
			</CardContent>
		</Card>
	</div>

	<!-- Account Information -->
	<Card>
		<CardHeader>
			<CardTitle>ข้อมูลบัญชี</CardTitle>
			<CardDescription>ข้อมูลบัญชีและสถานะการใช้งาน</CardDescription>
		</CardHeader>
		<CardContent>
			<div class="grid gap-4 md:grid-cols-2">
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
</div>
