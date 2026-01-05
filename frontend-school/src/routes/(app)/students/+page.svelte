<script lang="ts">
	import { onMount } from 'svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import { toast } from 'svelte-sonner';
	import { Users, Plus, Search, Filter } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	interface Student {
		id: string;
		first_name: string;
		last_name: string;
		student_id: string;
		grade_level: string;
		class_room: string;
		status: string;
	}

	let students = $state<Student[]>([]);
	let loading = $state(true);
	let searchTerm = $state('');
	let selectedGrade = $state('');
	let selectedClass = $state('');

	onMount(async () => {
		await loadStudents();
	});

	async function loadStudents() {
		loading = true;
		try {
			const params = new URLSearchParams();
			if (searchTerm) params.append('search', searchTerm);
			if (selectedGrade) params.append('grade_level', selectedGrade);
			if (selectedClass) params.append('class_room', selectedClass);

			const response = await fetch(`/api/students?${params.toString()}`, {
				headers: {
					Authorization: `Bearer ${localStorage.getItem('auth_token')}`
				}
			});

			if (response.ok) {
				const data = await response.json();
				students = data.data || [];
			} else {
				toast.error('ไม่สามารถโหลดข้อมูลนักเรียนได้');
			}
		} catch (error) {
			console.error('Failed to load students:', error);
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			loading = false;
		}
	}

	function handleSearch() {
		loadStudents();
	}

	function handleReset() {
		searchTerm = '';
		selectedGrade = '';
		selectedClass = '';
		loadStudents();
	}

	function goToNew() {
		goto(resolve('/students/new'));
	}

	function goToEdit(id: string) {
		goto(resolve(`/students/${id}/edit`));
	}
</script>

<svelte:head>
	<title>จัดการนักเรียน - SchoolOrbit</title>
</svelte:head>

<div class="container mx-auto p-6 space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold text-foreground">จัดการนักเรียน</h1>
			<p class="text-muted-foreground mt-1">จัดการข้อมูลนักเรียนทั้งหมด</p>
		</div>
		<Button onclick={goToNew}>
			<Plus class="w-4 h-4 mr-2" />
			เพิ่มนักเรียน
		</Button>
	</div>

	<!-- Search and Filter -->
	<Card class="p-6">
		<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
			<div class="md:col-span-2">
				<div class="relative">
					<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
					<Input
						type="text"
						placeholder="ค้นหาชื่อ หรือรหัสนักเรียน..."
						bind:value={searchTerm}
						class="pl-10"
						onkeydown={(e) => e.key === 'Enter' && handleSearch()}
					/>
				</div>
			</div>

			<div>
				<Input type="text" placeholder="ระดับชั้น (เช่น ม.1)" bind:value={selectedGrade} />
			</div>

			<div>
				<Input type="text" placeholder="ห้อง (เช่น 1, 2)" bind:value={selectedClass} />
			</div>
		</div>

		<div class="flex gap-2 mt-4">
			<Button onclick={handleSearch} variant="default">
				<Filter class="w-4 h-4 mr-2" />
				กรอง
			</Button>
			<Button onclick={handleReset} variant="outline">ล้างตัวกรอง</Button>
		</div>
	</Card>

	<!-- Students List -->
	<Card class="overflow-hidden">
		{#if loading}
			<div class="p-8 text-center">
				<div
					class="w-16 h-16 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"
				></div>
				<p class="text-muted-foreground">กำลังโหลดข้อมูล...</p>
			</div>
		{:else if students.length === 0}
			<div class="p-12 text-center">
				<Users class="w-16 h-16 text-muted-foreground/50 mx-auto mb-4" />
				<h3 class="text-lg font-semibold mb-2">ไม่พบนักเรียน</h3>
				<p class="text-muted-foreground mb-6">
					{searchTerm || selectedGrade || selectedClass
						? 'ไม่พบนักเรียนที่ตรงกับเงื่อนไขที่ค้นหา'
						: 'ยังไม่มีนักเรียนในระบบ เริ่มต้นโดยการเพิ่มนักเรียนคนแรก'}
				</p>
				{#if !searchTerm && !selectedGrade && !selectedClass}
					<Button onclick={goToNew}>
						<Plus class="w-4 h-4 mr-2" />
						เพิ่มนักเรียนคนแรก
					</Button>
				{/if}
			</div>
		{:else}
			<div class="overflow-x-auto">
				<table class="w-full">
					<thead class="bg-muted/50 border-b">
						<tr>
							<th class="px-6 py-4 text-left text-sm font-semibold">รหัสนักเรียน</th>
							<th class="px-6 py-4 text-left text-sm font-semibold">ชื่อ-นามสกุล</th>
							<th class="px-6 py-4 text-left text-sm font-semibold">ชั้น</th>
							<th class="px-6 py-4 text-left text-sm font-semibold">สถานะ</th>
							<th class="px-6 py-4 text-right text-sm font-semibold">จัดการ</th>
						</tr>
					</thead>
					<tbody class="divide-y">
						{#each students as student}
							<tr class="hover:bg-muted/30 transition-colors">
								<td class="px-6 py-4">
									<span class="font-mono text-sm">{student.student_id}</span>
								</td>
								<td class="px-6 py-4">
									<div class="flex items-center gap-3">
										<div
											class="w-10 h-10 bg-primary/10 rounded-full flex items-center justify-center"
										>
											<span class="text-sm font-semibold text-primary">
												{student.first_name.charAt(0)}
											</span>
										</div>
										<div>
											<p class="font-medium">{student.first_name} {student.last_name}</p>
										</div>
									</div>
								</td>
								<td class="px-6 py-4">
									{#if student.grade_level && student.class_room}
										<span class="text-sm">{student.grade_level}/{student.class_room}</span>
									{:else}
										<span class="text-sm text-muted-foreground">-</span>
									{/if}
								</td>
								<td class="px-6 py-4">
									{#if student.status === 'active'}
										<Badge variant="default" class="bg-green-500">ใช้งาน</Badge>
									{:else}
										<Badge variant="secondary">ไม่ใช้งาน</Badge>
									{/if}
								</td>
								<td class="px-6 py-4 text-right">
									<Button size="sm" variant="outline" onclick={() => goToEdit(student.id)}>
										ดูรายละเอียด
									</Button>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<!-- Summary -->
			<div class="px-6 py-4 bg-muted/30 border-t">
				<p class="text-sm text-muted-foreground">
					แสดง {students.length} รายการ
				</p>
			</div>
		{/if}
	</Card>
</div>
