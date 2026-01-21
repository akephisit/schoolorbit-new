<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		createAcademicYear,
		toggleActiveYear,
		createGradeLevel,
		deleteGradeLevel,
		getYearLevelConfig,
		saveYearLevelConfig
	} from '$lib/api/academic';
	import type { AcademicStructureData, GradeLevel } from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { 
		Loader2, 
		CalendarDays, 
		School, 
		Layers, 
		Plus, 
		Trash2, 
		BookOpen,
		Settings
	} from 'lucide-svelte';

	let loading = true;
	let structure: AcademicStructureData = { years: [], semesters: [], levels: [] };
	let activeYearLevelIds: string[] = [];
	
	// Year state
	let showCreateYearDialog = false;
	let isSubmitting = false;

	// Level state
	let showCreateLevelDialog = false;
	let isSubmittingLevel = false;
	let showDeleteLevelDialog = false;
	let levelToDelete: GradeLevel | null = null;
	let isDeletingLevel = false;

	// Year Config State
	let showConfigDialog = false;
	let configYear: AcademicStructureData['years'][0] | null = null;
	let configLevelIds: string[] = [];
	let isSavingConfig = false;

	async function openConfigDialog(year: AcademicStructureData['years'][0]) {
		configYear = year;
		isSavingConfig = false;
		configLevelIds = [];
		// Open first then load data (can show loading inside dialog)
		showConfigDialog = true;
		
		try {
			const res = await getYearLevelConfig(year.id);
			configLevelIds = res.data;
		} catch (error) {
			console.error(error);
			toast.error('ไม่สามารถโหลดข้อมูลการตั้งค่าระดับชั้นได้');
		}
	}

	async function saveConfig() {
		if (!configYear) return;
		isSavingConfig = true;
		try {
			await saveYearLevelConfig(configYear.id, configLevelIds);
			toast.success(`บันทึกชั้นเรียนสำหรับ ${configYear.name} เรียบร้อย`);
			showConfigDialog = false;
			// Reload data to update UI badges if we just edited the active year
			if (configYear.is_active) {
				await loadData();
			}
		} catch (error) {
			console.error(error);
			toast.error('บันทึกข้อมูลไม่สำเร็จ');
		} finally {
			isSavingConfig = false;
		}
	}

	function toggleConfigLevel(levelId: string, checked: boolean) {
		if (checked) {
			configLevelIds = [...configLevelIds, levelId];
		} else {
			configLevelIds = configLevelIds.filter((id) => id !== levelId);
		}
	}

	// New Year Form
	let newYear = {
		year: new Date().getFullYear() + 543,
		name: `ปีการศึกษา ${new Date().getFullYear() + 543}`,
		start_date: '',
		end_date: '',
		is_active: false
	};

	// New Level Form (year stored as string for Select compatibility)
	let newLevel = {
		level_type: '' as 'kindergarten' | 'primary' | 'secondary' | '',
		year: '1'
	};

	// Level type options for dropdown
	const levelTypeOptions = [
		{ value: 'kindergarten', label: 'อนุบาลศึกษา', prefix: 'อ.' },
		{ value: 'primary', label: 'ประถมศึกษา', prefix: 'ป.' },
		{ value: 'secondary', label: 'มัธยมศึกษา', prefix: 'ม.' }
	];

	// Get max years based on level type
	function getMaxYears(levelType: string): number {
		if (levelType === 'kindergarten') return 3;
		return 6; // primary and secondary
	}

	// Preview the generated name
	$: previewName = newLevel.level_type
		? `${levelTypeOptions.find((o) => o.value === newLevel.level_type)?.prefix}${newLevel.year}`
		: '';

	async function loadData() {
		try {
			const res = await getAcademicStructure();
			structure = res.data;

			// Fetch active levels for current active year
			const activeYear = structure.years.find((y) => y.is_active);
			if (activeYear) {
				const configRes = await getYearLevelConfig(activeYear.id);
				activeYearLevelIds = configRes.data;
			} else {
				activeYearLevelIds = [];
			}
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

	async function handleCreateLevel() {
		if (!newLevel.level_type || !newLevel.year) {
			toast.error('กรุณาเลือกประเภทและปีที่');
			return;
		}

		isSubmittingLevel = true;
		try {
			await createGradeLevel({
				level_type: newLevel.level_type as 'kindergarten' | 'primary' | 'secondary',
				year: parseInt(newLevel.year, 10)
			});
			toast.success('เพิ่มระดับชั้นเรียบร้อย');
			showCreateLevelDialog = false;
			await loadData();

			// Reset form
			newLevel = {
				level_type: '',
				year: '1'
			};
		} catch (error) {
			console.error(error);
			toast.error('เพิ่มระดับชั้นไม่สำเร็จ (ระดับชั้นนี้มีอยู่แล้ว)');
		} finally {
			isSubmittingLevel = false;
		}
	}

	function openDeleteLevelDialog(level: GradeLevel) {
		levelToDelete = level;
		showDeleteLevelDialog = true;
	}

	async function confirmDeleteLevel() {
		if (!levelToDelete) return;

		isDeletingLevel = true;
		try {
			await deleteGradeLevel(levelToDelete.id);
			toast.success('ลบระดับชั้นเรียบร้อย');
			showDeleteLevelDialog = false;
			levelToDelete = null;
			await loadData();
		} catch (error) {
			console.error(error);
			toast.error(error instanceof Error ? error.message : 'ลบระดับชั้นไม่สำเร็จ');
		} finally {
			isDeletingLevel = false;
		}
	}

	onMount(loadData);
</script>

<div class="space-y-6">
	<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<BookOpen class="w-8 h-8" />
				โครงสร้างวิชาการ
			</h2>
			<p class="text-muted-foreground mt-1">จัดการปีการศึกษา ภาคเรียน และระดับชั้น</p>
		</div>
		<div class="flex flex-wrap gap-2">
			<Button variant="outline" onclick={() => (showCreateLevelDialog = true)}>
				<Layers class="mr-2 h-4 w-4" />
				เพิ่มระดับชั้น
			</Button>
			<Button onclick={() => (showCreateYearDialog = true)}>
				<CalendarDays class="mr-2 h-4 w-4" />
				เพิ่มปีการศึกษา
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex h-40 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
			<!-- Academic Years Card -->
			<Card.Root class="md:col-span-2 min-w-0">
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<School class="h-5 w-5" />
						ปีการศึกษา (Academic Years)
					</Card.Title>
					<Card.Description>รายการปีการศึกษาทั้งหมดในระบบ</Card.Description>
				</Card.Header>
				<Card.Content class="pt-6">
					<div class="rounded-md border bg-card overflow-x-auto">
						<Table.Root class="min-w-[600px]">
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
										<Table.Cell class="font-medium">
											{year.name}
										</Table.Cell>
										<Table.Cell>
											{new Date(year.start_date).toLocaleDateString('th-TH')} - {new Date(
												year.end_date
											).toLocaleDateString('th-TH')}
										</Table.Cell>
										<Table.Cell>
											{#if year.is_active}
												<Badge variant="default" class="bg-green-500 hover:bg-green-600"
													>ปัจจุบัน</Badge
												>
											{:else}
												<Badge variant="outline">ประวัติ</Badge>
											{/if}
										</Table.Cell>
										<Table.Cell class="text-right">
											<div class="flex items-center justify-end gap-2">
												<Button
													variant="ghost"
													size="icon"
													onclick={() => openConfigDialog(year)}
													title="กำหนดชั้นเรียนที่เปิดสอน"
												>
													<Settings class="h-4 w-4" />
												</Button>
												{#if !year.is_active}
													<Button
														variant="outline"
														size="sm"
														onclick={() => handleToggleActive(year.id)}
													>
														ตั้งเป็นปีปัจจุบัน
													</Button>
												{/if}
											</div>
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

			<!-- Grade Levels Card -->
			<Card.Root>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<Layers class="h-5 w-5" />
						ระดับชั้นที่เปิดสอน
					</Card.Title>
					<Card.Description>ระดับชั้นมาตรฐานเรียงตามลำดับ</Card.Description>
				</Card.Header>
				<Card.Content>
					<div class="space-y-2">
						{#each structure.levels as level}
							<div
								class="flex items-center justify-between rounded-md border p-3 hover:bg-muted/50"
							>
								<div class="flex gap-3">
									<div
										class="flex h-8 w-8 items-center justify-center rounded-full bg-secondary text-xs font-bold"
									>
										{level.year}
									</div>
									<div>
										<div class="flex items-center gap-2">
											<p class="font-medium">{level.name}</p>
											{#if activeYearLevelIds.includes(level.id)}
												<Badge
													variant="outline"
													class="h-5 px-1.5 text-[10px] bg-green-50 text-green-700 border-green-200 dark:bg-green-900/20 dark:text-green-400 dark:border-green-800 pointer-events-none"
												>
													เปิดสอน
												</Badge>
											{/if}
										</div>
										<p class="text-xs text-muted-foreground">{level.code} • {level.short_name}</p>
									</div>
								</div>
								<Button
									variant="ghost"
									size="icon"
									class="h-8 w-8 text-muted-foreground hover:text-red-500"
									onclick={() => openDeleteLevelDialog(level)}
								>
									<Trash2 class="h-4 w-4" />
								</Button>
							</div>
						{/each}

						{#if structure.levels.length === 0}
							<div class="text-center py-6 text-muted-foreground text-sm">ยังไม่กำหนดระดับชั้น</div>
						{/if}

						<Button
							variant="outline"
							class="w-full mt-4"
							onclick={() => (showCreateLevelDialog = true)}
						>
							<Plus class="mr-2 h-4 w-4" />
							เพิ่มระดับชั้นใหม่
						</Button>
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
						<Label>วันเริ่มต้น</Label>
						<DatePicker bind:value={newYear.start_date} placeholder="วันเริ่มต้น" />
					</div>
					<div class="grid gap-2">
						<Label>วันสิ้นสุด</Label>
						<DatePicker bind:value={newYear.end_date} placeholder="วันสิ้นสุด" />
					</div>
				</div>
				<div class="flex items-center space-x-2">
					<Checkbox
						id="active"
						checked={newYear.is_active}
						onCheckedChange={(checked) => (newYear.is_active = checked === true)}
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

	<!-- Create Level Dialog -->
	<Dialog.Root bind:open={showCreateLevelDialog}>
		<Dialog.Content class="sm:max-w-[425px]">
			<Dialog.Header>
				<Dialog.Title>เพิ่มระดับชั้นใหม่</Dialog.Title>
				<Dialog.Description>เลือกประเภทและปีที่ของระดับชั้นที่ต้องการ</Dialog.Description>
			</Dialog.Header>
			<div class="grid gap-4 py-4">
				<div class="grid gap-2">
					<Label>ประเภทการศึกษา <span class="text-red-500">*</span></Label>
					<Select.Root type="single" bind:value={newLevel.level_type}>
						<Select.Trigger class="w-full">
							{levelTypeOptions.find((o) => o.value === newLevel.level_type)?.label ||
								'เลือกประเภท'}
						</Select.Trigger>
						<Select.Content>
							{#each levelTypeOptions as opt}
								<Select.Item value={opt.value}>{opt.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="grid gap-2">
					<Label>ปีที่ <span class="text-red-500">*</span></Label>
					<Select.Root type="single" bind:value={newLevel.year}>
						<Select.Trigger class="w-full">
							{`ปีที่ ${newLevel.year}`}
						</Select.Trigger>
						<Select.Content>
							{#each Array.from( { length: getMaxYears(newLevel.level_type) }, (_, i) => String(i + 1) ) as yr}
								<Select.Item value={yr}>ปีที่ {yr}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				{#if previewName}
					<div class="bg-muted/50 p-3 rounded-md text-sm">
						<p class="text-muted-foreground">ตัวอย่างชื่อที่จะสร้าง:</p>
						<p class="font-bold text-lg">{previewName}</p>
					</div>
				{/if}
			</div>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showCreateLevelDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleCreateLevel} disabled={isSubmittingLevel || !newLevel.level_type}>
					{#if isSubmittingLevel}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					บันทึก
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Level Confirmation Dialog -->
	<Dialog.Root bind:open={showDeleteLevelDialog}>
		<Dialog.Content class="sm:max-w-[400px]">
			<Dialog.Header>
				<Dialog.Title class="flex items-center gap-2 text-red-600">
					<Trash2 class="h-5 w-5" />
					ยืนยันการลบระดับชั้น
				</Dialog.Title>
				<Dialog.Description>
					การลบระดับชั้นจะไม่สามารถย้อนกลับได้
					หากมีห้องเรียนหรือนักเรียนเชื่อมโยงอยู่จะไม่สามารถลบได้
				</Dialog.Description>
			</Dialog.Header>

			{#if levelToDelete}
				<div class="py-4">
					<div
						class="flex items-center gap-3 p-4 bg-red-50 border border-red-200 rounded-lg dark:bg-red-950/20 dark:border-red-900"
					>
						<div
							class="flex h-10 w-10 items-center justify-center rounded-full bg-red-500 text-white text-sm font-bold"
						>
							{levelToDelete.year}
						</div>
						<div>
							<p class="font-semibold text-red-800 dark:text-red-200">{levelToDelete.name}</p>
							<p class="text-sm text-red-600 dark:text-red-400">
								{levelToDelete.code} • {levelToDelete.short_name}
							</p>
						</div>
					</div>
				</div>
			{/if}

			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => {
						showDeleteLevelDialog = false;
						levelToDelete = null;
					}}
				>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmDeleteLevel} disabled={isDeletingLevel}>
					{#if isDeletingLevel}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					ยืนยันลบ
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
	<!-- Year Config Dialog -->
	<Dialog.Root bind:open={showConfigDialog}>
		<Dialog.Content class="sm:max-w-[500px]">
			<Dialog.Header>
				<Dialog.Title>กำหนดชั้นเรียนที่เปิดสอน</Dialog.Title>
				<Dialog.Description>
					เลือกชั้นเรียนที่จะเปิดสอนในปีการศึกษา <strong>{configYear?.name}</strong>
				</Dialog.Description>
			</Dialog.Header>

			<div class="py-4">
				{#if !configYear}
					<div class="flex justify-center p-4"><Loader2 class="animate-spin" /></div>
				{:else}
					<div class="grid grid-cols-2 gap-4 max-h-[60vh] overflow-y-auto pr-2">
						{#each structure.levels as level}
							<div
								class="flex items-center space-x-2 border p-2 rounded-md hover:bg-muted/50 transition-colors"
							>
								<Checkbox
									id={`level-${level.id}`}
									checked={configLevelIds.includes(level.id)}
									onCheckedChange={(c) => toggleConfigLevel(level.id, c === true)}
								/>
								<Label for={`level-${level.id}`} class="cursor-pointer flex-1 user-select-none">
									<span class="font-bold">{level.short_name}</span>
									<span class="text-muted-foreground text-xs ml-1">({level.name})</span>
								</Label>
							</div>
						{/each}
					</div>
				{/if}
			</div>

			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showConfigDialog = false)}>ยกเลิก</Button>
				<Button onclick={saveConfig} disabled={isSavingConfig}>
					{#if isSavingConfig}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					บันทึกการเปลี่ยนแปลง
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
