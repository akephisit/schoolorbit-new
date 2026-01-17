<script lang="ts">
	import { onMount } from 'svelte';
	import { getAcademicStructure, createAcademicYear, toggleActiveYear } from '$lib/api/academic';
	import type { AcademicStructureData } from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import CalendarDays from 'lucide-svelte/icons/calendar-days';
	import School from 'lucide-svelte/icons/school';
	import Layers from 'lucide-svelte/icons/layers';

	let loading = true;
	let structure: AcademicStructureData = { years: [], semesters: [], levels: [] };
	let showCreateYearDialog = false;
	let isSubmitting = false;

	// New Year Form
	let newYear = {
		year: new Date().getFullYear() + 543,
		name: `ปีการศึกษา ${new Date().getFullYear() + 543}`,
		start_date: '',
		end_date: '',
		is_active: false
	};

	async function loadData() {
		try {
			const res = await getAcademicStructure();
			structure = res.data;
		} catch (error) {
			console.error(error);
			toast.error('ไม่สามารถโหลดข้อมูลโครงสร้างวิชาการได้');
		} finally {
			loading = false;
		}
	}

	async function handleCreateYear() {
		if (!newYear.year || !newYear.start_date || !newYear.end_date) {
			toast.error('กรุณากรอกข้อมูลให้ครบถ้วน');
			return;
		}

		isSubmitting = true;
		try {
			await createAcademicYear(newYear);
			toast.success('สร้างปีการศึกษาสำเร็จ');
			showCreateYearDialog = false;
			await loadData();
			// Reset form
			newYear = {
				year: new Date().getFullYear() + 543 + 1,
				name: `ปีการศึกษา ${new Date().getFullYear() + 543 + 1}`,
				start_date: '',
				end_date: '',
				is_active: false
			};
		} catch (error) {
			console.error(error);
			toast.error('เกิดข้อผิดพลาดในการสร้างปีการศึกษา');
		} finally {
			isSubmitting = false;
		}
	}

	async function handleToggleActive(id: string) {
		try {
			await toggleActiveYear(id);
			toast.success('อัปเดตสถานะปีการศึกษาเรียบร้อย');
			await loadData();
		} catch (error) {
			console.error(error);
			toast.error('เกิดข้อผิดพลาด');
		}
	}

	onMount(loadData);
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h2 class="text-2xl font-bold tracking-tight">โครงสร้างวิชาการ</h2>
			<p class="text-muted-foreground">จัดการปีการศึกษา ภาคเรียน และระดับชั้น</p>
		</div>
		<Button onclick={() => (showCreateYearDialog = true)}>
			<CalendarDays class="mr-2 h-4 w-4" />
			เพิ่มปีการศึกษา
		</Button>
	</div>

	{#if loading}
		<div class="flex h-40 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
			<!-- Academic Years Card -->
			<Card.Root class="md:col-span-2">
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<School class="h-5 w-5" />
						ปีการศึกษา (Academic Years)
					</Card.Title>
					<Card.Description>รายการปีการศึกษาทั้งหมดในระบบ</Card.Description>
				</Card.Header>
				<Card.Content>
					<div class="rounded-md border">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head>ปีการศึกษา</Table.Head>
									<Table.Head>ช่วงเวลา</Table.Head>
									<Table.Head>สถานะ</Table.Head>
									<Table.Head class="text-right">จัดการ</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each structure.years as year}
									<Table.Row>
										<Table.Cell class="font-medium">{year.name}</Table.Cell>
										<Table.Cell>
											{new Date(year.start_date).toLocaleDateString('th-TH')} - {new Date(
												year.end_date
											).toLocaleDateString('th-TH')}
										</Table.Cell>
										<Table.Cell>
											{#if year.is_active}
												<Badge variant="default" class="bg-green-500">ปัจจุบัน</Badge>
											{:else}
												<Badge variant="outline">ประวัติ</Badge>
											{/if}
										</Table.Cell>
										<Table.Cell class="text-right">
											{#if !year.is_active}
												<Button
													variant="ghost"
													size="sm"
													onclick={() => handleToggleActive(year.id)}
												>
													ตั้งเป็นปีปัจจุบัน
												</Button>
											{/if}
										</Table.Cell>
									</Table.Row>
								{/each}
								{#if structure.years.length === 0}
									<Table.Row>
										<Table.Cell colspan={4} class="h-24 text-center">
											ไม่พบข้อมูลปีการศึกษา
										</Table.Cell>
									</Table.Row>
								{/if}
							</Table.Body>
						</Table.Root>
					</div>
				</Card.Content>
			</Card.Root>

			<!-- Grade Levels Card (Read-only summary for now) -->
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<Layers class="h-5 w-5" />
						ระดับชั้นที่เปิดสอน
					</Card.Title>
					<Card.Description>ข้อมูลระดับชั้นมาตรฐาน</Card.Description>
				</Card.Header>
				<Card.Content>
					<div class="space-y-4">
						{#each structure.levels as level}
							<div class="flex items-center justify-between border-b pb-2 last:border-0">
								<div>
									<p class="font-medium">{level.name}</p>
									<p class="text-xs text-muted-foreground">{level.code}</p>
								</div>
								<Badge variant="secondary">{level.short_name}</Badge>
							</div>
						{/each}
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}

	<!-- Create Year Dialog -->
	<Dialog.Root bind:open={showCreateYearDialog}>
		<Dialog.Content class="sm:max-w-[425px]">
			<Dialog.Header>
				<Dialog.Title>เพิ่มปีการศึกษาใหม่</Dialog.Title>
				<Dialog.Description>สร้างปีการศึกษาใหม่เพื่อกำหนดโครงสร้างห้องเรียน</Dialog.Description>
			</Dialog.Header>
			<div class="grid gap-4 py-4">
				<div class="grid gap-2">
					<Label for="year">ปีพ.ศ.</Label>
					<Input id="year" type="number" bind:value={newYear.year} />
				</div>
				<div class="grid gap-2">
					<Label for="name">ชื่อเรียก</Label>
					<Input id="name" bind:value={newYear.name} />
				</div>
				<div class="grid grid-cols-2 gap-4">
					<div class="grid gap-2">
						<Label for="start">วันเริ่มต้น</Label>
						<Input id="start" type="date" bind:value={newYear.start_date} />
					</div>
					<div class="grid gap-2">
						<Label for="end">วันสิ้นสุด</Label>
						<Input id="end" type="date" bind:value={newYear.end_date} />
					</div>
				</div>
				<div class="flex items-center space-x-2">
					<input
						type="checkbox"
						id="active"
						bind:checked={newYear.is_active}
						class="h-4 w-4 rounded border-gray-300"
					/>
					<Label for="active">ตั้งเป็นปีการศึกษาปัจจุบันทันที</Label>
				</div>
			</div>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showCreateYearDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleCreateYear} disabled={isSubmitting}>
					{#if isSubmitting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					บันทึก
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
