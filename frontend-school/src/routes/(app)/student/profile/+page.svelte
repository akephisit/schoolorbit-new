<script lang="ts">
	import { onMount } from 'svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { toast } from 'svelte-sonner';
	import { User, Edit, Save, X } from 'lucide-svelte';
	import { getOwnProfile, updateOwnProfile } from '$lib/api/students';

	let student = $state<any>(null);
	let loading = $state(true);
	let editing = $state(false);
	let saving = $state(false);

	// Editable fields
	let phone = $state('');
	let address = $state('');
	let nickname = $state('');

	onMount(async () => {
		await loadProfile();
	});

	async function loadProfile() {
		loading = true;
		try {
			const response = await getOwnProfile();
			student = response.data;

			// Initialize editable fields
			phone = student.phone || '';
			address = student.address || '';
			nickname = student.nickname || '';
		} catch (error) {
			console.error('Failed to load profile:', error);
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		saving = true;
		try {
			await updateOwnProfile({
				phone,
				address,
				nickname
			});
			toast.success('บันทึกข้อมูลสำเร็จ');
			editing = false;
			await loadProfile();
		} catch (error) {
			console.error('Failed to save profile:', error);
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			saving = false;
		}
	}

	function handleCancel() {
		// Reset to original values
		phone = student.phone || '';
		address = student.address || '';
		nickname = student.nickname || '';
		editing = false;
	}
</script>

<svelte:head>
	<title>ข้อมูลส่วนตัว - Student Portal</title>
</svelte:head>

<div class="container mx-auto p-6 max-w-4xl space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold text-foreground">ข้อมูลส่วนตัว</h1>
			<p class="text-muted-foreground mt-1">ดูและแก้ไขข้อมูลส่วนตัวของคุณ</p>
		</div>

		{#if !editing && !loading}
			<Button onclick={() => (editing = true)}>
				<Edit class="w-4 h-4 mr-2" />
				แก้ไขข้อมูล
			</Button>
		{/if}
	</div>

	{#if loading}
		<Card class="p-6">
			<div class="space-y-4">
				{#each Array(6) as _}
					<div class="animate-pulse">
						<div class="h-4 bg-muted rounded w-1/4 mb-2"></div>
						<div class="h-10 bg-muted rounded"></div>
					</div>
				{/each}
			</div>
		</Card>
	{:else if student}
		<!-- Basic Information (Read-only) -->
		<Card class="p-6">
			<div class="flex items-center gap-3 mb-6">
				<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
					<User class="w-5 h-5 text-primary" />
				</div>
				<h2 class="text-xl font-semibold">ข้อมูลพื้นฐาน</h2>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
				<div class="space-y-2">
					<Label>ชื่อ-นามสกุล</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{student.title || ''}
						{student.first_name}
						{student.last_name}
					</div>
				</div>

				<div class="space-y-2">
					<Label>รหัสนักเรียน</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{student.student_id || '-'}
					</div>
				</div>

				<div class="space-y-2">
					<Label>ระดับชั้น</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{#if student.grade_level && student.class_room}
							{student.grade_level}/{student.class_room}
						{:else}
							-
						{/if}
					</div>
				</div>

				<div class="space-y-2">
					<Label>เพศ</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{#if student.gender === 'male'}
							ชาย
						{:else if student.gender === 'female'}
							หญิง
						{:else}
							-
						{/if}
					</div>
				</div>

				<div class="space-y-2">
					<Label>วันเกิด</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{student.date_of_birth || '-'}
					</div>
				</div>

				<div class="space-y-2">
					<Label>อีเมล</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
						{student.email || '-'}
					</div>
				</div>
			</div>

			<p class="text-sm text-muted-foreground mt-4">
				ข้อมูลเหล่านี้ไม่สามารถแก้ไขได้ หากพบข้อผิดพลาดกรุณาติดต่อผู้ดูแลระบบ
			</p>
		</Card>

		<!-- Editable Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลติดต่อ</h2>

			{#if editing}
				<div class="space-y-6">
					<div class="space-y-2">
						<Label for="nickname">ชื่อเล่น</Label>
						<Input
							id="nickname"
							type="text"
							bind:value={nickname}
							placeholder="ชื่อเล่น"
							disabled={saving}
						/>
					</div>

					<div class="space-y-2">
						<Label for="phone">เบอร์โทรศัพท์</Label>
						<Input
							id="phone"
							type="tel"
							bind:value={phone}
							placeholder="0812345678"
							disabled={saving}
						/>
					</div>

					<div class="space-y-2">
						<Label for="address">ที่อยู่</Label>
						<Textarea
							id="address"
							bind:value={address}
							placeholder="ที่อยู่ปัจจุบัน"
							rows={4}
							disabled={saving}
						/>
					</div>

					<div class="flex gap-3">
						<Button onclick={handleSave} disabled={saving} class="flex-1">
							{#if saving}
								กำลังบันทึก...
							{:else}
								<Save class="w-4 h-4 mr-2" />
								บันทึก
							{/if}
						</Button>
						<Button variant="outline" onclick={handleCancel} disabled={saving}>
							<X class="w-4 h-4 mr-2" />
							ยกเลิก
						</Button>
					</div>
				</div>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
					<div class="space-y-2">
						<Label>ชื่อเล่น</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.nickname || '-'}
						</div>
					</div>

					<div class="space-y-2">
						<Label>เบอร์โทรศัพท์</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
							{student.phone || '-'}
						</div>
					</div>

					<div class="space-y-2 md:col-span-2">
						<Label>ที่อยู่</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground min-h-[80px]">
							{student.address || '-'}
						</div>
					</div>
				</div>
			{/if}
		</Card>

		<!-- Medical Information (Read-only) -->
		{#if student.blood_type || student.allergies || student.medical_conditions}
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-6">ข้อมูลสุขภาพ</h2>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
					{#if student.blood_type}
						<div class="space-y-2">
							<Label>หมู่เลือด</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
								{student.blood_type}
							</div>
						</div>
					{/if}

					{#if student.allergies}
						<div class="space-y-2 md:col-span-2">
							<Label>อาการแพ้</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
								{student.allergies}
							</div>
						</div>
					{/if}

					{#if student.medical_conditions}
						<div class="space-y-2 md:col-span-2">
							<Label>โรคประจำตัว</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md text-foreground">
								{student.medical_conditions}
							</div>
						</div>
					{/if}
				</div>

				<p class="text-sm text-muted-foreground mt-4">
					ข้อมูลสุขภาพไม่สามารถแก้ไขได้ ติดต่อผู้ดูแลระบบหากต้องการเปลี่ยนแปลง
				</p>
			</Card>
		{/if}
	{/if}
</div>
