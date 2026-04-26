<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listTimetableTemplates,
		deleteTimetableTemplate,
		createTemplateFromCurrent,
		applyTimetableTemplate,
		clearTimetable,
		type TimetableTemplateView
	} from '$lib/api/scheduling';
	import { getAcademicStructure, type Semester } from '$lib/api/academic';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { FileStack, Plus, Trash2, Play, Eraser, LoaderCircle } from 'lucide-svelte';

	let { data } = $props();

	let loading = $state(true);
	let templates = $state<TimetableTemplateView[]>([]);
	let semesters = $state<Semester[]>([]);
	let selectedSemesterId = $state('');

	// Create-from-current dialog
	let showCreateDialog = $state(false);
	let createName = $state('');
	let createDescription = $state('');
	let creating = $state(false);

	// Apply dialog
	let showApplyDialog = $state(false);
	let applyTarget = $state<TimetableTemplateView | null>(null);
	let applying = $state(false);

	// Clear dialog
	let showClearDialog = $state(false);
	let clearMode = $state<'all_except_course' | 'course_only' | 'all'>('all_except_course');
	let clearing = $state(false);

	async function loadAll() {
		loading = true;
		try {
			const [tplRes, structRes] = await Promise.all([
				listTimetableTemplates(),
				getAcademicStructure()
			]);
			templates = tplRes.data ?? [];
			const yrs = structRes.data.years;
			const activeYr = yrs.find((y) => y.is_active) ?? yrs[0];
			if (activeYr) {
				semesters = (structRes.data.semesters ?? []).filter(
					(s) => s.academic_year_id === activeYr.id
				);
				const activeSem = semesters.find((s) => s.is_active) ?? semesters[0];
				if (activeSem) selectedSemesterId = activeSem.id;
			}
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!createName.trim()) {
			toast.error('กรุณาระบุชื่อ template');
			return;
		}
		if (!selectedSemesterId) {
			toast.error('กรุณาเลือกภาคเรียนที่จะ snapshot');
			return;
		}
		creating = true;
		try {
			await createTemplateFromCurrent({
				semester_id: selectedSemesterId,
				name: createName.trim(),
				description: createDescription.trim() || undefined
			});
			toast.success('สร้าง template สำเร็จ');
			showCreateDialog = false;
			createName = '';
			createDescription = '';
			await loadAll();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'สร้างไม่สำเร็จ');
		} finally {
			creating = false;
		}
	}

	async function handleDelete(t: TimetableTemplateView) {
		if (!window.confirm(`ลบ "${t.name}"? — ไม่สามารถกู้คืนได้`)) return;
		try {
			await deleteTimetableTemplate(t.id);
			toast.success('ลบสำเร็จ');
			await loadAll();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		}
	}

	function openApply(t: TimetableTemplateView) {
		applyTarget = t;
		showApplyDialog = true;
	}

	async function handleApply() {
		if (!applyTarget || !selectedSemesterId) return;
		applying = true;
		try {
			const res = await applyTimetableTemplate(applyTarget.id, {
				semester_id: selectedSemesterId
			});
			toast.success(`Apply สำเร็จ — เพิ่ม ${res.data?.applied ?? 0} entries`);
			showApplyDialog = false;
			applyTarget = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Apply ไม่สำเร็จ');
		} finally {
			applying = false;
		}
	}

	async function handleClear() {
		if (!selectedSemesterId) {
			toast.error('กรุณาเลือกภาคเรียน');
			return;
		}
		const types =
			clearMode === 'all'
				? ['BREAK', 'HOMEROOM', 'ACTIVITY', 'ACADEMIC', 'COURSE']
				: clearMode === 'course_only'
					? ['COURSE']
					: undefined; // default: all except COURSE
		clearing = true;
		try {
			const res = await clearTimetable({
				semester_id: selectedSemesterId,
				entry_types: types
			});
			toast.success(`เคลียร์สำเร็จ — ลบ ${res.data?.deleted ?? 0} entries`);
			showClearDialog = false;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เคลียร์ไม่สำเร็จ');
		} finally {
			clearing = false;
		}
	}

	function formatDate(s: string): string {
		return new Date(s).toLocaleString('th-TH', { dateStyle: 'short', timeStyle: 'short' });
	}

	onMount(loadAll);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<div class="container mx-auto p-4 space-y-4">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<FileStack class="w-6 h-6 text-primary" />
			<h1 class="text-2xl font-bold">Templates ตาราง</h1>
		</div>
		<div class="flex items-center gap-2">
			<Select.Root type="single" bind:value={selectedSemesterId}>
				<Select.Trigger class="w-[200px]">
					{semesters.find((s) => s.id === selectedSemesterId)?.name || 'เลือกภาคเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each semesters as sem (sem.id)}
						<Select.Item value={sem.id}>{sem.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Button variant="outline" onclick={() => (showClearDialog = true)} disabled={!selectedSemesterId}>
				<Eraser class="w-4 h-4 mr-2" />
				เคลียร์ตาราง
			</Button>
			<Button onclick={() => (showCreateDialog = true)} disabled={!selectedSemesterId}>
				<Plus class="w-4 h-4 mr-2" />
				สร้างจากตารางปัจจุบัน
			</Button>
		</div>
	</div>

	<Card.Root class="p-3 bg-muted/30">
		<p class="text-sm text-muted-foreground">
			💡 <strong>Workflow:</strong> 1) Batch fixed slots (พัก/โฮมรูม/sync) ที่หน้าตาราง →
			2) สร้าง template จากตารางปัจจุบัน → 3) ถ้าจัดอัตโนมัติแล้วไม่ถูกใจ → เคลียร์ → apply
			template → ลองจัดใหม่
		</p>
	</Card.Root>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
		</div>
	{:else if templates.length === 0}
		<Card.Root class="p-8 text-center text-muted-foreground">
			<FileStack class="w-12 h-12 mx-auto mb-3 opacity-30" />
			<p>ยังไม่มี template</p>
			<p class="text-xs mt-1">กด "สร้างจากตารางปัจจุบัน" เพื่อ snapshot ตาราง semester ปัจจุบัน</p>
		</Card.Root>
	{:else}
		<div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
			{#each templates as t (t.id)}
				<Card.Root class="p-4">
					<div class="flex items-start justify-between mb-2">
						<div class="flex-1 min-w-0">
							<h3 class="font-semibold truncate">{t.name}</h3>
							{#if t.description}
								<p class="text-xs text-muted-foreground line-clamp-2 mt-1">
									{t.description}
								</p>
							{/if}
						</div>
					</div>
					<div class="text-xs text-muted-foreground space-y-0.5">
						<div>📋 {t.entry_count} entries</div>
						<div>📅 {formatDate(t.created_at)}</div>
					</div>
					<div class="flex gap-1 mt-3">
						<Button
							size="sm"
							variant="default"
							onclick={() => openApply(t)}
							disabled={!selectedSemesterId}
							class="flex-1"
						>
							<Play class="w-3 h-3 mr-1" />
							ใช้ template
						</Button>
						<Button size="sm" variant="ghost" onclick={() => handleDelete(t)}>
							<Trash2 class="w-3 h-3 text-destructive" />
						</Button>
					</div>
				</Card.Root>
			{/each}
		</div>
	{/if}
</div>

<!-- Create-from-current dialog -->
<Dialog.Root bind:open={showCreateDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>สร้าง Template จากตารางปัจจุบัน</Dialog.Title>
			<Dialog.Description>
				Snapshot entries ที่ไม่ใช่ COURSE (พัก/โฮมรูม/กิจกรรม) ของ
				<strong>{semesters.find((s) => s.id === selectedSemesterId)?.name || ''}</strong>
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div>
				<Label>ชื่อ template</Label>
				<Input bind:value={createName} placeholder="เช่น ตาราง ม.ต้น 2/2569" />
			</div>
			<div>
				<Label>คำอธิบาย (optional)</Label>
				<Input bind:value={createDescription} placeholder="เช่น พักเช้า โฮมรูม ชุมนุม sync" />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showCreateDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleCreate} disabled={creating}>
				{#if creating}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{/if}
				สร้าง
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Apply dialog -->
<Dialog.Root bind:open={showApplyDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ใช้ Template</Dialog.Title>
			<Dialog.Description>
				เพิ่ม entries จาก <strong>{applyTarget?.name}</strong> เข้า
				<strong>{semesters.find((s) => s.id === selectedSemesterId)?.name || ''}</strong>
			</Dialog.Description>
		</Dialog.Header>
		<p class="text-sm text-muted-foreground py-2">
			Entries ที่ทับกับของเดิมจะถูกข้าม (ON CONFLICT DO NOTHING)
			— ถ้าต้องการเริ่มใหม่ทั้งหมด กดเคลียร์ก่อน
		</p>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showApplyDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleApply} disabled={applying}>
				{#if applying}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{/if}
				Apply
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Clear dialog -->
<Dialog.Root bind:open={showClearDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>เคลียร์ตาราง</Dialog.Title>
			<Dialog.Description>
				ลบ entries ใน <strong>{semesters.find((s) => s.id === selectedSemesterId)?.name || ''}</strong>
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-2 py-2">
			<label class="flex items-center gap-2 cursor-pointer">
				<input type="radio" bind:group={clearMode} value="all_except_course" />
				<span class="text-sm">ลบกิจกรรม/พัก/โฮมรูม (เก็บวิชา)</span>
			</label>
			<label class="flex items-center gap-2 cursor-pointer">
				<input type="radio" bind:group={clearMode} value="course_only" />
				<span class="text-sm">ลบเฉพาะวิชา (เก็บกิจกรรม/พัก/โฮมรูม) — รีจัดใหม่</span>
			</label>
			<label class="flex items-center gap-2 cursor-pointer">
				<input type="radio" bind:group={clearMode} value="all" />
				<span class="text-sm text-destructive">ลบทุกอย่าง — ระวัง!</span>
			</label>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showClearDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleClear} disabled={clearing}>
				{#if clearing}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
				{/if}
				เคลียร์
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
