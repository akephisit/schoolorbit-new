<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	let { data } = $props();

	import {
		type AcademicPeriod,
		listPeriods,
		createPeriod,
		updatePeriod,
		deletePeriod,
		reorderPeriods
	} from '$lib/api/timetable';
	import { lookupAcademicYears, type LookupItem } from '$lib/api/academic';

	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	import {
		Plus,
		Settings,
		Trash2,
		LoaderCircle,
		Calendar,
		GripVertical,
		Info
	} from 'lucide-svelte';

	let loading = $state(true);
	let periods = $state<AcademicPeriod[]>([]);
	let academicYears = $state<LookupItem[]>([]);
	let selectedYearId = $state('');

	let showPeriodDialog = $state(false);
	let showDeleteDialog = $state(false);
	let submitting = $state(false);

	let editingPeriod = $state<AcademicPeriod | null>(null);
	let deleteTarget = $state<{ id: string; name: string } | null>(null);

	let formYearId = $state('');

	// Drag-and-drop state
	let draggedPeriod = $state<AcademicPeriod | null>(null);
	let isDirty = $state(false);

	const canReadAcademicPeriods = $derived($can.has(PERMISSIONS.ACADEMIC_STRUCTURE_READ_ALL));
	const canManageAcademicPeriods = $derived($can.has(PERMISSIONS.ACADEMIC_STRUCTURE_MANAGE_ALL));

	async function loadData() {
		if (!canReadAcademicPeriods) {
			academicYears = [];
			periods = [];
			loading = false;
			return;
		}

		try {
			loading = true;
			const yearsRes = await lookupAcademicYears(false);
			academicYears = yearsRes.data;

			if (academicYears.length > 0 && !selectedYearId) {
				const activeYear = academicYears.find((y) => y.is_current) || academicYears[0];
				selectedYearId = activeYear.id;
			}

			if (selectedYearId) {
				await loadPeriods();
			}
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadPeriods() {
		if (!canReadAcademicPeriods || !selectedYearId) return;
		try {
			const res = await listPeriods({ academic_year_id: selectedYearId });
			periods = res.data.sort((a, b) => a.order_index - b.order_index);
			isDirty = false;
		} catch {
			toast.error('โหลดคาบเวลาไม่สำเร็จ');
		}
	}

	async function handleSavePeriod(e: SubmitEvent) {
		e.preventDefault();

		if (!canManageAcademicPeriods) {
			toast.error('ไม่มีสิทธิ์จัดการคาบเวลา');
			return;
		}

		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);

		const payload = {
			academic_year_id: formData.get('academic_year_id') as string,
			name: formData.get('name') as string,
			start_time: formData.get('start_time') as string,
			end_time: formData.get('end_time') as string
			// order_index ไม่ส่ง — backend จะ auto MAX+1 ตอน create
			// edit ก็ไม่แตะ order_index (ใช้ drag-drop แทน)
		};

		submitting = true;
		try {
			if (editingPeriod) {
				await updatePeriod(editingPeriod.id, payload);
				toast.success('บันทึกข้อมูลสำเร็จ');
			} else {
				await createPeriod(payload);
				toast.success('เพิ่มคาบเวลาสำเร็จ');
			}
			showPeriodDialog = false;
			await loadPeriods();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	async function handleDelete() {
		if (!canManageAcademicPeriods) {
			toast.error('ไม่มีสิทธิ์ลบคาบเวลา');
			return;
		}
		if (!deleteTarget) return;
		submitting = true;
		try {
			await deletePeriod(deleteTarget.id);
			toast.success('ลบคาบเวลาสำเร็จ');
			showDeleteDialog = false;
			await loadPeriods();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ (อาจมีข้อมูลตารางสอนเชื่อมโยง)');
		} finally {
			submitting = false;
		}
	}

	function openAddPeriod() {
		if (!canManageAcademicPeriods) {
			toast.error('ไม่มีสิทธิ์เพิ่มคาบเวลา');
			return;
		}

		editingPeriod = null;
		formYearId = selectedYearId;
		showPeriodDialog = true;
	}

	function openEditPeriod(p: AcademicPeriod) {
		if (!canManageAcademicPeriods) {
			toast.error('ไม่มีสิทธิ์แก้ไขคาบเวลา');
			return;
		}

		editingPeriod = p;
		formYearId = p.academic_year_id;
		showPeriodDialog = true;
	}

	function confirmDelete(p: AcademicPeriod) {
		if (!canManageAcademicPeriods) {
			toast.error('ไม่มีสิทธิ์ลบคาบเวลา');
			return;
		}

		const label = p.name || `${formatTime(p.start_time)} – ${formatTime(p.end_time)}`;
		deleteTarget = { id: p.id, name: label };
		showDeleteDialog = true;
	}

	function formatTime(time: string): string {
		return time.substring(0, 5);
	}

	// =========================================
	// Drag & Drop
	// =========================================

	function handleDragStart(e: DragEvent, p: AcademicPeriod) {
		if (!canManageAcademicPeriods) return;
		e.dataTransfer!.effectAllowed = 'move';
		draggedPeriod = p;
	}

	function handleDragOver(e: DragEvent) {
		if (!canManageAcademicPeriods) return;
		e.preventDefault();
		e.dataTransfer!.dropEffect = 'move';
	}

	function handleDragEnter(_e: DragEvent, target: AcademicPeriod) {
		if (!canManageAcademicPeriods) return;
		if (!draggedPeriod || draggedPeriod.id === target.id) return;

		const oldIndex = periods.findIndex((p) => p.id === draggedPeriod!.id);
		const newIndex = periods.findIndex((p) => p.id === target.id);
		if (oldIndex === -1 || newIndex === -1) return;

		const next = [...periods];
		const [removed] = next.splice(oldIndex, 1);
		next.splice(newIndex, 0, removed);
		periods = next;
		isDirty = true;
	}

	async function handleDragEnd() {
		const dragged = draggedPeriod;
		draggedPeriod = null;
		if (!canManageAcademicPeriods || !isDirty || !dragged || !selectedYearId) return;

		const items = periods.map((p, i) => ({ id: p.id, order_index: i + 1 }));
		try {
			await reorderPeriods(selectedYearId, items);
			// Update local order_index ให้ตรง backend (จะ reload เพื่อความ accurate)
			await loadPeriods();
			toast.success('บันทึกลำดับสำเร็จ');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกลำดับไม่สำเร็จ');
			await loadPeriods(); // revert
		}
	}

	$effect(() => {
		if (selectedYearId) {
			loadPeriods();
		}
	});

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="ตั้งค่าคาบเวลา"
	description="กำหนดคาบเรียนมาตรฐานของโรงเรียนในแต่ละปีการศึกษา (ใช้สำหรับจัดตารางสอน)"
>
	<div class="flex flex-wrap items-center gap-4">
		<div class="w-[250px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full">
					<Calendar class="mr-2 h-4 w-4" />
					{academicYears.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year (year.id)}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		{#if canManageAcademicPeriods}
			<div class="ml-auto">
				<Button onclick={openAddPeriod} disabled={!selectedYearId}>
					<Plus class="mr-2 h-4 w-4" /> เพิ่มคาบเวลา
				</Button>
			</div>
		{/if}
	</div>

	{#if !canReadAcademicPeriods}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูคาบเวลา"
			description="บัญชีนี้เข้า module โครงสร้างวิชาการได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลคาบเวลา"
		/>
	{:else if canManageAcademicPeriods}
		<div
			class="bg-muted/40 text-muted-foreground flex items-start gap-2 rounded-md border p-3 text-sm"
		>
			<Info class="mt-0.5 h-4 w-4 shrink-0" />
			<span>
				ลากที่ <GripVertical class="inline h-3.5 w-3.5" /> เพื่อจัดลำดับคาบ — ตารางสอนที่จัดไปแล้วจะไม่ได้รับผลกระทบ
				(เปลี่ยนแค่ลำดับการแสดงผล)
			</span>
		</div>
	{/if}

	{#if canReadAcademicPeriods}
		{#if loading}
			<PageSkeleton variant="cards" rows={3} />
		{:else if periods.length === 0}
			<PageState
				title={selectedYearId ? 'ยังไม่มีคาบเวลา' : 'ยังไม่ได้เลือกปีการศึกษา'}
				description={selectedYearId
					? 'กดปุ่มเพิ่มคาบเวลาเพื่อเริ่มต้นกำหนดคาบเรียน'
					: 'กรุณาเลือกปีการศึกษาเพื่อดูคาบเวลา'}
				actionLabel={selectedYearId && canManageAcademicPeriods ? 'เพิ่มคาบเวลา' : undefined}
				onaction={selectedYearId && canManageAcademicPeriods ? openAddPeriod : undefined}
			/>
		{:else}
			<Card.Root>
				<div class="divide-border divide-y" role="list">
					{#each periods as p, i (p.id)}
						<div
							role="listitem"
							draggable={canManageAcademicPeriods}
							ondragstart={(e) => handleDragStart(e, p)}
							ondragover={handleDragOver}
							ondragenter={(e) => handleDragEnter(e, p)}
							ondragend={handleDragEnd}
							class="hover:bg-muted/30 flex items-center gap-3 px-4 py-3 transition-colors {draggedPeriod?.id ===
							p.id
								? 'opacity-40'
								: ''}"
							style="touch-action: none;"
						>
							{#if canManageAcademicPeriods}
								<div
									class="text-muted-foreground hover:text-foreground cursor-grab active:cursor-grabbing"
								>
									<GripVertical class="h-5 w-5" />
								</div>
							{/if}

							<Badge variant="outline" class="font-mono">#{i + 1}</Badge>

							<div class="min-w-0 flex-1">
								{#if p.name}
									<p class="text-foreground truncate font-medium">{p.name}</p>
									<p class="text-muted-foreground text-sm">
										{formatTime(p.start_time)} – {formatTime(p.end_time)}
									</p>
								{:else}
									<p class="text-foreground font-medium">
										{formatTime(p.start_time)} – {formatTime(p.end_time)}
									</p>
								{/if}
							</div>

							<Badge variant={p.is_active ? 'default' : 'outline'}>
								{p.is_active ? 'ใช้งาน' : 'ไม่ใช้งาน'}
							</Badge>

							{#if canManageAcademicPeriods}
								<div class="flex items-center gap-1">
									<Button variant="ghost" size="icon" onclick={() => openEditPeriod(p)}>
										<Settings class="h-4 w-4" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										class="text-destructive"
										onclick={() => confirmDelete(p)}
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			</Card.Root>
		{/if}
	{/if}

	<!-- Period Dialog -->
	{#if canManageAcademicPeriods}
		<Dialog.Root bind:open={showPeriodDialog}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title>{editingPeriod ? 'แก้ไขคาบเวลา' : 'เพิ่มคาบเวลาใหม่'}</Dialog.Title>
				</Dialog.Header>
				<form onsubmit={handleSavePeriod} class="space-y-4 py-4">
					<input type="hidden" name="academic_year_id" value={formYearId} />

					<div class="space-y-2">
						<Label>ชื่อคาบ <span class="text-muted-foreground text-xs">(ไม่บังคับ)</span></Label>
						<Input
							name="name"
							value={editingPeriod?.name || ''}
							placeholder="เช่น พักเที่ยง, โฮมรูม (เว้นว่างถ้าเป็นคาบเรียนปกติ)"
						/>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label>เวลาเริ่ม <span class="text-red-500">*</span></Label>
							<Input
								type="time"
								name="start_time"
								value={editingPeriod?.start_time ? formatTime(editingPeriod.start_time) : ''}
								required
							/>
						</div>
						<div class="space-y-2">
							<Label>เวลาจบ <span class="text-red-500">*</span></Label>
							<Input
								type="time"
								name="end_time"
								value={editingPeriod?.end_time ? formatTime(editingPeriod.end_time) : ''}
								required
							/>
						</div>
					</div>

					{#if !editingPeriod}
						<p class="text-muted-foreground text-xs">
							ลำดับคาบจะถูกกำหนดเป็นตัวสุดท้ายอัตโนมัติ — ลากเพื่อจัดลำดับใหม่หลังเพิ่ม
						</p>
					{/if}

					<Dialog.Footer>
						<Button variant="outline" type="button" onclick={() => (showPeriodDialog = false)}
							>ยกเลิก</Button
						>
						<Button type="submit" disabled={submitting}>บันทึก</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>
	{/if}

	<!-- Delete Confirm -->
	{#if canManageAcademicPeriods}
		<Dialog.Root bind:open={showDeleteDialog}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title>ยืนยันการลบ</Dialog.Title>
					<Dialog.Description>
						คุณต้องการลบคาบ "{deleteTarget?.name}" ใช่หรือไม่?
						หากมีตารางสอนที่ใช้คาบนี้จะไม่สามารถลบได้
					</Dialog.Description>
				</Dialog.Header>
				<Dialog.Footer>
					<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
					<Button variant="destructive" onclick={handleDelete} disabled={submitting}
						>ยืนยันลบ</Button
					>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog.Root>
	{/if}
</PageShell>
