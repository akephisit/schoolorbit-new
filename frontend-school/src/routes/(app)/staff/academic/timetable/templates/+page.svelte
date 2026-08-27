<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		applyTimetableTemplate,
		clearTimetable,
		createTimetableTemplateFromCurrent,
		deleteTimetableTemplate,
		listTimetableTemplates,
		type TimetableTemplate
	} from '$lib/api/timetable';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Eraser, Play, Plus, Trash2 } from 'lucide-svelte';

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	let templates = $state<TimetableTemplate[]>([]);
	let loading = $state(false);
	let creating = $state(false);
	let applying = $state(false);
	let clearing = $state(false);
	let deletingTemplateId = $state<string | null>(null);
	let showCreateDialog = $state(false);
	let showApplyDialog = $state(false);
	let showClearDialog = $state(false);
	let applyTarget = $state<TimetableTemplate | null>(null);
	let createName = $state('');
	let createDescription = $state('');
	let clearMode = $state<'all_except_course' | 'course_only' | 'all'>('all_except_course');
	let errorMessage = $state('');

	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL
		)
	);
	const canManage = $derived($can.has(PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL));
	const hasDirtyDraft = $derived(
		showCreateDialog && Boolean(createName.trim() || createDescription.trim())
	);

	async function loadTemplates(): Promise<void> {
		loading = true;
		errorMessage = '';
		try {
			templates = await listTimetableTemplates();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดแม่แบบตารางสอนไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	async function handleCreate(): Promise<void> {
		if (!academicTermId || !createName.trim()) return;
		creating = true;
		try {
			const created = await createTimetableTemplateFromCurrent({
				academicTermId,
				name: createName.trim(),
				description: createDescription.trim() || null,
				entryTypes: null
			});
			templates = [created, ...templates.filter((template) => template.id !== created.id)];
			showCreateDialog = false;
			createName = '';
			createDescription = '';
			toast.success('สร้างแม่แบบจากตารางของภาคเรียนนี้แล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างแม่แบบไม่สำเร็จ');
		} finally {
			creating = false;
		}
	}

	async function handleDelete(template: TimetableTemplate): Promise<void> {
		deletingTemplateId = template.id;
		try {
			await deleteTimetableTemplate(template.id);
			templates = templates.filter((item) => item.id !== template.id);
			toast.success('ลบแม่แบบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลบแม่แบบไม่สำเร็จ');
		} finally {
			deletingTemplateId = null;
		}
	}

	function openApply(template: TimetableTemplate): void {
		applyTarget = template;
		showApplyDialog = true;
	}

	async function handleApply(): Promise<void> {
		if (!academicTermId || !applyTarget) return;
		applying = true;
		try {
			const result = await applyTimetableTemplate(applyTarget.id, { academicTermId });
			showApplyDialog = false;
			applyTarget = null;
			toast.success(`นำแม่แบบไปใช้แล้ว ${result.applied} คาบ`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'นำแม่แบบไปใช้ไม่สำเร็จ');
		} finally {
			applying = false;
		}
	}

	async function handleClear(): Promise<void> {
		if (!academicTermId) return;
		const entryTypes =
			clearMode === 'all'
				? ['BREAK', 'HOMEROOM', 'ACTIVITY', 'ACADEMIC', 'COURSE']
				: clearMode === 'course_only'
					? ['COURSE']
					: ['BREAK', 'HOMEROOM', 'ACTIVITY', 'ACADEMIC'];
		clearing = true;
		try {
			const removed = await clearTimetable({ academicTermId, entryTypes });
			showClearDialog = false;
			toast.success(`ล้างออกจากตารางแล้ว ${removed.length} คาบ`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล้างตารางไม่สำเร็จ');
		} finally {
			clearing = false;
		}
	}

	function formatDate(value: string): string {
		return new Date(value).toLocaleString('th-TH', { dateStyle: 'short', timeStyle: 'short' });
	}

	onMount(() => {
		const unregisterDirty = registerAcademicContextDirtySource(
			'timetable-template-draft',
			() => hasDirtyDraft
		);
		void loadTemplates();
		return unregisterDirty;
	});
</script>

<PageShell
	title="แม่แบบตารางสอน"
	description="เก็บรูปแบบตารางไว้ใช้ซ้ำ แล้วนำไปใช้กับภาคเรียนที่เลือกบนแถบด้านบน"
	backHref="/staff/academic/timetable"
>
	{#snippet actions()}
		{#if canManage && academicTermId}
			<div class="flex flex-wrap gap-2">
				<Button variant="outline" onclick={() => (showClearDialog = true)}
					><Eraser /> ล้างตาราง</Button
				>
				<Button onclick={() => (showCreateDialog = true)}><Plus /> สร้างจากภาคนี้</Button>
			</div>
		{/if}
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูแม่แบบ"
			description="ต้องมีสิทธิ์อ่านชุดการเรียนระดับโรงเรียน"
		/>
	{:else if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="แม่แบบไม่ผูกกับภาค แต่การสร้าง นำไปใช้ และล้างตารางต้องมีภาคเรียนเป้าหมายที่ชัดเจน"
		/>
	{:else if loading}
		<PageSkeleton variant="cards" rows={3} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดแม่แบบไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadTemplates}
		/>
	{:else if templates.length === 0}
		<PageState
			title="ยังไม่มีแม่แบบ"
			description="สร้างแม่แบบจากตารางของภาคเรียนที่เลือกเพื่อใช้เป็นจุดเริ่มต้นในภาคถัดไป"
			actionLabel={canManage ? 'สร้างจากภาคนี้' : undefined}
			onaction={canManage ? () => (showCreateDialog = true) : undefined}
		/>
	{:else}
		<div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
			{#each templates as template (template.id)}
				<Card.Root>
					<Card.Header>
						<Card.Title>{template.name}</Card.Title>
						<Card.Description>{template.description ?? 'ไม่มีคำอธิบาย'}</Card.Description>
					</Card.Header>
					<Card.Content>
						<p class="text-muted-foreground text-xs">สร้างเมื่อ {formatDate(template.createdAt)}</p>
					</Card.Content>
					{#if canManage}
						<Card.Footer class="gap-2">
							<Button class="flex-1" onclick={() => openApply(template)}
								><Play /> ใช้กับภาคนี้</Button
							>
							<LoadingButton
								variant="ghost"
								size="icon"
								loading={deletingTemplateId === template.id}
								loadingLabel=""
								aria-label={`ลบ ${template.name}`}
								onclick={() => handleDelete(template)}
								><Trash2 class="text-destructive" /></LoadingButton
							>
						</Card.Footer>
					{/if}
				</Card.Root>
			{/each}
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={showCreateDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>สร้างแม่แบบจากภาคเรียนนี้</Dialog.Title>
			<Dialog.Description
				>ระบบจะบันทึกตำแหน่งตามลำดับคาบและ stable resource เพื่อปรับใช้กับภาคอื่นได้</Dialog.Description
			>
		</Dialog.Header>
		<div class="space-y-4 py-2">
			<div class="space-y-2">
				<Label for="template-name">ชื่อแม่แบบ</Label><Input
					id="template-name"
					bind:value={createName}
					placeholder="เช่น ตารางพื้นฐาน ม.ต้น"
				/>
			</div>
			<div class="space-y-2">
				<Label for="template-description">คำอธิบาย</Label><Input
					id="template-description"
					bind:value={createDescription}
				/>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showCreateDialog = false)}>ยกเลิก</Button>
			<LoadingButton
				loading={creating}
				loadingLabel="กำลังสร้าง"
				disabled={!createName.trim()}
				onclick={handleCreate}>สร้างแม่แบบ</LoadingButton
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showApplyDialog}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>ใช้แม่แบบ “{applyTarget?.name}”</Dialog.Title><Dialog.Description
				>รายการจะถูกจับคู่กับตารางเวลา กลุ่มเรียน และทรัพยากรของภาคเรียนที่เลือก
				หากจับคู่ไม่ได้ระบบจะหยุดโดยไม่สร้าง compatibility record</Dialog.Description
			></Dialog.Header
		>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (showApplyDialog = false)}>ยกเลิก</Button
			><LoadingButton loading={applying} loadingLabel="กำลังนำไปใช้" onclick={handleApply}
				>ยืนยัน</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={showClearDialog}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>ล้างตารางของภาคเรียนนี้</Dialog.Title><Dialog.Description
				>เลือกชนิดคาบที่จะปิดใช้งาน การทำงานนี้ไม่ลบชุดการเรียนหรือรายชื่อนักเรียน</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-2 py-2">
			<Label for="clear-mode">ขอบเขต</Label>
			<Select.Root type="single" bind:value={clearMode}>
				<Select.Trigger id="clear-mode" class="w-full">
					{clearMode === 'all_except_course'
						? 'คาบทั่วไปและกิจกรรม (เก็บรายวิชา)'
						: clearMode === 'course_only'
							? 'เฉพาะรายวิชา'
							: 'ทุกคาบ'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="all_except_course">คาบทั่วไปและกิจกรรม (เก็บรายวิชา)</Select.Item>
					<Select.Item value="course_only">เฉพาะรายวิชา</Select.Item>
					<Select.Item value="all">ทุกคาบ</Select.Item>
				</Select.Content>
			</Select.Root>
		</div>
		<Dialog.Footer
			><Button variant="outline" onclick={() => (showClearDialog = false)}>ยกเลิก</Button
			><LoadingButton
				variant="destructive"
				loading={clearing}
				loadingLabel="กำลังล้าง"
				onclick={handleClear}>ล้างตามขอบเขต</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
