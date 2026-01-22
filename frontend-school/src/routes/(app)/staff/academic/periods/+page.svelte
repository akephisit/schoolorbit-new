<script lang="ts">
    import { onMount } from 'svelte';
    import { toast } from 'svelte-sonner';
    import {
        type AcademicPeriod,
        listPeriods,
        createPeriod,
        updatePeriod,
        deletePeriod
    } from '$lib/api/timetable';
    import { lookupAcademicYears } from '$lib/api/academic';
    
    import * as Card from '$lib/components/ui/card';
    import * as Table from '$lib/components/ui/table';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { Badge } from '$lib/components/ui/badge';
    import * as Dialog from '$lib/components/ui/dialog';
    import * as Select from '$lib/components/ui/select';
    
    import {
        Clock,
        Plus,
        Settings,
        Trash2,
        Loader2,
        Calendar
    } from 'lucide-svelte';

    const PERIOD_TYPES = [
        { value: 'TEACHING', label: 'เรียน', color: 'bg-blue-100 text-blue-800' },
        { value: 'BREAK', label: 'พัก', color: 'bg-green-100 text-green-800' },
        { value: 'ACTIVITY', label: 'กิจกรรม', color: 'bg-purple-100 text-purple-800' },
        { value: 'HOMEROOM', label: 'โฮมรูม', color: 'bg-orange-100 text-orange-800' }
    ];

    // State
    let loading = $state(true);
    let periods = $state<AcademicPeriod[]>([]);
    let academicYears = $state<any[]>([]);
    let selectedYearId = $state('');
    
    // Dialogs
    let showPeriodDialog = $state(false);
    let showDeleteDialog = $state(false);
    let submitting = $state(false);

    // Editing
    let editingPeriod = $state<AcademicPeriod | null>(null);
    let deleteTarget = $state<{ id: string; name: string } | null>(null);
    
    // Form state for Select
    let formYearId = $state('');
    let formPeriodType = $state('TEACHING');

    async function loadData() {
        try {
            loading = true;
            const yearsRes = await lookupAcademicYears(false);
            academicYears = yearsRes.data;
            
            if (academicYears.length > 0 && !selectedYearId) {
                const activeYear = academicYears.find(y => y.is_current) || academicYears[0];
                selectedYearId = activeYear.id;
            }
            
            if (selectedYearId) {
                await loadPeriods();
            }
        } catch (e) {
            toast.error('โหลดข้อมูลไม่สำเร็จ');
        } finally {
            loading = false;
        }
    }

    async function loadPeriods() {
        if (!selectedYearId) return;
        try {
            const res = await listPeriods({ academic_year_id: selectedYearId });
            periods = res.data;
        } catch (e) {
            toast.error('โหลดคาบเวลาไม่สำเร็จ');
        }
    }

    async function handleSavePeriod(e: SubmitEvent) {
        e.preventDefault();
        const form = e.target as HTMLFormElement;
        const formData = new FormData(form);
        
        const payload = {
            academic_year_id: formData.get('academic_year_id') as string,
            name: formData.get('name') as string,
            start_time: formData.get('start_time') as string,
            end_time: formData.get('end_time') as string,
            type: formData.get('type') as string,
            order_index: parseInt(formData.get('order_index') as string)
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
            loadPeriods();
        } catch (e: any) {
            toast.error(e.message || 'บันทึกไม่สำเร็จ');
        } finally {
            submitting = false;
        }
    }

    async function handleDelete() {
        if (!deleteTarget) return;
        submitting = true;
        try {
            await deletePeriod(deleteTarget.id);
            toast.success('ลบคาบเวลาสำเร็จ');
            showDeleteDialog = false;
            loadPeriods();
        } catch (e: any) {
            toast.error(e.message || 'ลบไม่สำเร็จ (อาจมีข้อมูลตารางสอนเชื่อมโยง)');
        } finally {
            submitting = false;
        }
    }

    function openAddPeriod() {
        editingPeriod = null;
        formYearId = selectedYearId;
        formPeriodType = 'TEACHING';
        showPeriodDialog = true;
    }

    function openEditPeriod(p: AcademicPeriod) {
        editingPeriod = p;
        formYearId = p.academic_year_id;
        formPeriodType = p.type;
        showPeriodDialog = true;
    }

    function confirmDelete(p: AcademicPeriod) {
        deleteTarget = { id: p.id, name: p.name };
        showDeleteDialog = true;
    }

    function formatTime(time: string): string {
        // Convert "HH:MM:SS" to "HH:MM"
        return time.substring(0, 5);
    }

    $effect(() => {
        if (selectedYearId) {
            loadPeriods();
        }
    });

    onMount(loadData);
</script>

<div class="space-y-6">
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<Clock class="w-8 h-8" />
			ตั้งค่าคาบเวลา
		</h2>
		<p class="text-muted-foreground">
			กำหนดคาบเรียนมาตรฐานของโรงเรียนในแต่ละปีการศึกษา (ใช้สำหรับจัดตารางสอน)
		</p>
	</div>

	<div class="flex items-center gap-4 flex-wrap">
		<div class="w-[250px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full">
					<Calendar class="w-4 h-4 mr-2" />
					{academicYears.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
		<div class="ml-auto">
			<Button onclick={openAddPeriod} disabled={!selectedYearId}>
				<Plus class="w-4 h-4 mr-2" /> เพิ่มคาบเวลา
			</Button>
		</div>
	</div>

	<Card.Root>
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="w-[60px]">ลำดับ</Table.Head>
					<Table.Head>ชื่อคาบ</Table.Head>
					<Table.Head>ประเภท</Table.Head>
					<Table.Head>เวลาเริ่ม</Table.Head>
					<Table.Head>เวลาจบ</Table.Head>
					<Table.Head class="w-[100px]">สถานะ</Table.Head>
					<Table.Head class="text-right">จัดการ</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#if loading}
					<Table.Row
						><Table.Cell colspan={7} class="h-24 text-center"
							><Loader2 class="animate-spin mx-auto" /></Table.Cell
						></Table.Row
					>
				{:else if periods.length === 0}
					<Table.Row
						><Table.Cell colspan={7} class="h-24 text-center text-muted-foreground">
							{selectedYearId
								? 'ยังไม่มีคาบเวลา กดปุ่ม "เพิ่มคาบเวลา" เพื่อเริ่มต้น'
								: 'กรุณาเลือกปีการศึกษา'}
						</Table.Cell></Table.Row
					>
				{:else}
					{#each periods as p}
						<Table.Row>
							<Table.Cell class="font-bold text-center">{p.order_index}</Table.Cell>
							<Table.Cell class="font-medium">{p.name}</Table.Cell>
							<Table.Cell>
								{@const typeInfo = PERIOD_TYPES.find((t) => t.value === p.type)}
								<Badge variant="outline" class={typeInfo?.color || ''}>
									{typeInfo?.label || p.type}
								</Badge>
							</Table.Cell>
							<Table.Cell>{formatTime(p.start_time)}</Table.Cell>
							<Table.Cell>{formatTime(p.end_time)}</Table.Cell>
							<Table.Cell>
								<Badge variant={p.is_active ? 'default' : 'outline'}>
									{p.is_active ? 'ใช้งาน' : 'ไม่ใช้งาน'}
								</Badge>
							</Table.Cell>
							<Table.Cell class="text-right">
								<Button variant="ghost" size="icon" onclick={() => openEditPeriod(p)}>
									<Settings class="w-4 h-4" />
								</Button>
								<Button
									variant="ghost"
									size="icon"
									class="text-destructive"
									onclick={() => confirmDelete(p)}
								>
									<Trash2 class="w-4 h-4" />
								</Button>
							</Table.Cell>
						</Table.Row>
					{/each}
				{/if}
			</Table.Body>
		</Table.Root>
	</Card.Root>

	<!-- Period Dialog -->
	<Dialog.Root bind:open={showPeriodDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>{editingPeriod ? 'แก้ไขคาบเวลา' : 'เพิ่มคาบเวลาใหม่'}</Dialog.Title>
			</Dialog.Header>
			<form onsubmit={handleSavePeriod} class="space-y-4 py-4">
				<input type="hidden" name="academic_year_id" value={formYearId} />
				<input type="hidden" name="type" value={formPeriodType} />

				<div class="grid grid-cols-2 gap-4">
					<div class="col-span-1 space-y-2">
						<Label>ลำดับคาบ <span class="text-red-500">*</span></Label>
						<Input
							type="number"
							name="order_index"
							value={editingPeriod?.order_index?.toString() || ''}
							required
							min="1"
						/>
					</div>
					<div class="col-span-1 space-y-2">
						<Label>ประเภท</Label>
						<Select.Root type="single" bind:value={formPeriodType}>
							<Select.Trigger class="w-full">
								{PERIOD_TYPES.find((t) => t.value === formPeriodType)?.label}
							</Select.Trigger>
							<Select.Content>
								{#each PERIOD_TYPES as t}
									<Select.Item value={t.value}>{t.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<div class="space-y-2">
					<Label>ชื่อคาบ <span class="text-red-500">*</span></Label>
					<Input
						name="name"
						value={editingPeriod?.name || ''}
						required
						placeholder="เช่น คาบที่ 1, พักเที่ยง"
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

				<Dialog.Footer>
					<Button variant="outline" type="button" onclick={() => (showPeriodDialog = false)}
						>ยกเลิก</Button
					>
					<Button type="submit" disabled={submitting}>บันทึก</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Confirm -->
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
				<Button variant="destructive" onclick={handleDelete} disabled={submitting}>ยืนยันลบ</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
