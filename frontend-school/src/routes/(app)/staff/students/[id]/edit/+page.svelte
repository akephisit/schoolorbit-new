<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Edit, Save, X, Trash2 } from 'lucide-svelte';
	import * as Select from '$lib/components/ui/select';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { getStudent, updateStudent, deleteStudent } from '$lib/api/students';

	let studentId = $derived($page.params.id as string);
	let student = $state<any>(null);
	let loading = $state(true);
	let editing = $state(false);
	let saving = $state(false);
	let deleting = $state(false);

	// Form data
	let formData = $state({
		email: '',
		first_name: '',
		last_name: '',
		phone: '',
		address: '',
		grade_level: '',
		class_room: '',
		student_number: null as number | null
	});

	onMount(async () => {
		await loadStudent();
	});

	async function loadStudent() {
		loading = true;
		try {
			const response = await getStudent(studentId);
			student = response.data;

			// Initialize form data
			formData = {
				email: student.email || '',
				first_name: student.first_name || '',
				last_name: student.last_name || '',
				phone: student.phone || '',
				address: student.address || '',
				grade_level: student.grade_level || '',
				class_room: student.class_room || '',
				student_number: student.student_number || null
			};
		} catch (error) {
			console.error('Failed to load student:', error);
			const message = error instanceof Error ? error.message : 'ไม่พบนักเรียน';
			toast.error(message);
			goto(resolve('/staff/students'));
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		saving = true;
		try {
			await updateStudent(studentId, {
				...formData,
				student_number: formData.student_number || undefined
			});
			toast.success('บันทึกข้อมูลสำเร็จ');
			editing = false;
			await loadStudent();
		} catch (error) {
			console.error('Failed to save:', error);
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			saving = false;
		}
	}

	function handleCancel() {
		// Reset to original values
		formData = {
			email: student.email || '',
			first_name: student.first_name || '',
			last_name: student.last_name || '',
			phone: student.phone || '',
			address: student.address || '',
			grade_level: student.grade_level || '',
			class_room: student.class_room || '',
			student_number: student.student_number || null
		};
		editing = false;
	}

	async function handleDelete() {
		if (!confirm('คุณแน่ใจหรือไม่ที่จะลบนักเรียนคนนี้?')) {
			return;
		}

		deleting = true;
		try {
			await deleteStudent(studentId);
			toast.success('ลบนักเรียนสำเร็จ');
			goto(resolve('/staff/students'));
		} catch (error) {
			console.error('Failed to delete:', error);
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			deleting = false;
		}
	}
</script>

<svelte:head>
	<title>
		{student ? `${student.first_name} ${student.last_name}` : 'นักเรียน'} - SchoolOrbit
	</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button href="/staff/students" variant="ghost" size="sm">
				<ArrowLeft class="w-4 h-4" />
			</Button>
			<div>
				<h1 class="text-2xl font-bold text-foreground">
					{#if student}
						{student.first_name} {student.last_name}
					{:else}
						นักเรียน
					{/if}
				</h1>
				<p class="text-sm text-muted-foreground">รายละเอียดและจัดการข้อมูลนักเรียน</p>
			</div>
		</div>

		{#if !editing && !loading}
			<div class="flex gap-2">
				<Button onclick={() => (editing = true)}>
					<Edit class="w-4 h-4 mr-2" />
					แก้ไข
				</Button>
				<Button variant="destructive" onclick={handleDelete} disabled={deleting}>
					{#if deleting}
						กำลังลบ...
					{:else}
						<Trash2 class="w-4 h-4 mr-2" />
						ลบ
					{/if}
				</Button>
			</div>
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
		<!-- Student ID & Status -->
		<Card class="p-6">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-muted-foreground">รหัสนักเรียน</p>
					<p class="text-2xl font-bold">{student.student_id}</p>
				</div>
				<Badge
					variant={student.status === 'active' ? 'default' : 'secondary'}
					class={student.status === 'active' ? 'bg-green-500' : ''}
				>
					{student.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
				</Badge>
			</div>
		</Card>

		<!-- Basic Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลพื้นฐาน</h2>

			{#if editing}
				<div class="space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div>
							<Label for="first_name">ชื่อ</Label>
							<Input
								id="first_name"
								type="text"
								bind:value={formData.first_name}
								disabled={saving}
							/>
						</div>

						<div>
							<Label for="last_name">นามสกุล</Label>
							<Input id="last_name" type="text" bind:value={formData.last_name} disabled={saving} />
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div>
							<Label for="email">อีเมล</Label>
							<Input id="email" type="email" bind:value={formData.email} disabled={saving} />
						</div>

						<div>
							<Label for="phone">เบอร์โทรศัพท์</Label>
							<Input id="phone" type="tel" bind:value={formData.phone} disabled={saving} />
						</div>
					</div>

					<div>
						<Label for="address">ที่อยู่</Label>
						<Input id="address" type="text" bind:value={formData.address} disabled={saving} />
					</div>
				</div>
			{:else}
				<div class="grid grid-cols-2 gap-6">
					<div>
						<Label>ชื่อ-นามสกุล</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.title || ''}
							{student.first_name}
							{student.last_name}
						</div>
					</div>

					<div>
						<Label>เพศ</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.gender === 'male' ? 'ชาย' : student.gender === 'female' ? 'หญิง' : '-'}
						</div>
					</div>

					<div>
						<Label>วันเกิด</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.date_of_birth || '-'}
						</div>
					</div>

					<div>
						<Label>เลขบัตรประชาชน</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.national_id || '-'}
						</div>
					</div>

					<div>
						<Label>อีเมล</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.email || '-'}
						</div>
					</div>

					<div>
						<Label>เบอร์โทรศัพท์</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.phone || '-'}
						</div>
					</div>

					<div class="col-span-2">
						<Label>ที่อยู่</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.address || '-'}
						</div>
					</div>
				</div>
			{/if}
		</Card>

		<!-- Student Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลนักเรียน</h2>

			{#if editing}
				<div class="space-y-4">
					<div class="grid grid-cols-3 gap-4">
						<div>
							<Label for="grade_level">ระดับชั้น</Label>
							<Input
								id="grade_level"
								type="text"
								bind:value={formData.grade_level}
								placeholder="ม.1"
								disabled={saving}
							/>
						</div>

						<div>
							<Label for="class_room">ห้อง</Label>
							<Input
								id="class_room"
								type="text"
								bind:value={formData.class_room}
								placeholder="1"
								disabled={saving}
							/>
						</div>

						<div>
							<Label for="student_number">เลขที่</Label>
							<Input
								id="student_number"
								type="number"
								bind:value={formData.student_number}
								placeholder="1"
								disabled={saving}
							/>
						</div>
					</div>

					<div class="flex gap-3 mt-6">
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
				<div class="grid grid-cols-3 gap-6">
					<div>
						<Label>ระดับชั้น</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.grade_level || '-'}
						</div>
					</div>

					<div>
						<Label>ห้อง</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.class_room || '-'}
						</div>
					</div>

					<div>
						<Label>เลขที่</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.student_number || '-'}
						</div>
					</div>
				</div>
			{/if}
		</Card>

		<!-- Medical Information (if any) -->
		{#if student.blood_type || student.allergies || student.medical_conditions}
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-6">ข้อมูลสุขภาพ</h2>

				<div class="grid grid-cols-2 gap-6">
					{#if student.blood_type}
						<div>
							<Label>หมู่เลือด</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.blood_type}
							</div>
						</div>
					{/if}

					{#if student.allergies}
						<div class="col-span-2">
							<Label>อาการแพ้</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.allergies}
							</div>
						</div>
					{/if}

					{#if student.medical_conditions}
						<div class="col-span-2">
							<Label>โรคประจำตัว</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.medical_conditions}
							</div>
						</div>
					{/if}
				</div>
			</Card>
		{/if}
	{/if}
</div>
