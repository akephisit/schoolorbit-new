<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listInstructorConstraints,
		updateInstructorConstraints,
		reorderInstructorPriority,
		getSchoolSettings,
		updateSchoolSettings,
		listPeriods,
		type InstructorConstraintView,
		type Period,
		type TimeSlot
	} from '$lib/api/scheduling';
	import { getAcademicStructure, getSchoolDays, type AcademicYear } from '$lib/api/academic';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { GripVertical, ChevronDown, ChevronRight, Sparkles, Save, LoaderCircle } from 'lucide-svelte';

	let { data } = $props();

	let loading = $state(true);
	let saving = $state(false);
	let instructors = $state<InstructorConstraintView[]>([]);
	let periods = $state<Period[]>([]);
	let schoolDays = $state<{ value: string; label: string; shortLabel: string }[]>([]);
	let defaultMaxConsecutive = $state(4);
	let activeYear = $state<AcademicYear | null>(null);

	// Per-row UI state
	let expandedIds = $state(new Set<string>());
	// Local edits — keyed by instructor_id, only flushed on Save
	let unavailableEdits = $state(new Map<string, TimeSlot[]>());

	// DnD state
	let draggedId = $state<string | null>(null);
	let priorityDirty = $state(false);

	function slotKey(day: string, periodId: string): string {
		return `${day}__${periodId}`;
	}

	function isUnavailable(instructorId: string, day: string, periodId: string): boolean {
		const slots = unavailableEdits.get(instructorId);
		if (!slots) return false;
		return slots.some((s) => s.day === day && s.period_id === periodId);
	}

	function toggleUnavailable(instructorId: string, day: string, periodId: string) {
		const current = unavailableEdits.get(instructorId) ?? [];
		const idx = current.findIndex((s) => s.day === day && s.period_id === periodId);
		const next = idx >= 0
			? current.filter((_, i) => i !== idx)
			: [...current, { day, period_id: periodId }];
		const newMap = new Map(unavailableEdits);
		newMap.set(instructorId, next);
		unavailableEdits = newMap;
	}

	async function loadAll() {
		loading = true;
		try {
			const struct = await getAcademicStructure();
			const yrs = struct.data.years;
			activeYear = yrs.find((y) => y.is_active) ?? yrs[0] ?? null;
			if (!activeYear) {
				toast.error('ไม่พบปีการศึกษาที่ใช้งานอยู่');
				return;
			}
			schoolDays = getSchoolDays(activeYear.school_days);

			const [instrRes, periodsRes, settingsRes] = await Promise.all([
				listInstructorConstraints(),
				listPeriods(activeYear.id),
				getSchoolSettings()
			]);
			instructors = (instrRes.data ?? []).filter((i) => i.primary_course_count > 0);
			periods = (periodsRes.data ?? []).sort((a, b) => a.order_index - b.order_index);
			defaultMaxConsecutive = settingsRes.data?.default_max_consecutive ?? 4;

			// Initialize edits from server state
			const init = new Map<string, TimeSlot[]>();
			for (const i of instructors) {
				init.set(i.id, (i.hard_unavailable_slots ?? []) as TimeSlot[]);
			}
			unavailableEdits = init;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	// =========================================
	// Drag & Drop priority
	// =========================================

	function onDragStart(e: DragEvent, id: string) {
		e.dataTransfer!.effectAllowed = 'move';
		draggedId = id;
	}

	function onDragOver(e: DragEvent) {
		e.preventDefault();
		e.dataTransfer!.dropEffect = 'move';
	}

	function onDragEnter(_e: DragEvent, targetId: string) {
		if (!draggedId || draggedId === targetId) return;
		const src = instructors.findIndex((i) => i.id === draggedId);
		const dst = instructors.findIndex((i) => i.id === targetId);
		if (src < 0 || dst < 0) return;
		const next = [...instructors];
		const [moved] = next.splice(src, 1);
		next.splice(dst, 0, moved);
		instructors = next;
		priorityDirty = true;
	}

	function onDragEnd() {
		draggedId = null;
	}

	// =========================================
	// Save
	// =========================================

	async function saveAll() {
		if (saving) return;
		saving = true;
		try {
			const ops: Promise<unknown>[] = [];

			// 1. Priority order — bulk endpoint (1 query batch)
			if (priorityDirty) {
				ops.push(reorderInstructorPriority(instructors.map((i) => i.id)));
			}

			// 2. Global settings
			ops.push(updateSchoolSettings({ default_max_consecutive: defaultMaxConsecutive }));

			// 3. Per-instructor unavailable — only ที่เปลี่ยนจริง
			for (const i of instructors) {
				const local = unavailableEdits.get(i.id) ?? [];
				const remote = (i.hard_unavailable_slots ?? []) as TimeSlot[];
				if (slotsEqual(local, remote)) continue;
				ops.push(updateInstructorConstraints(i.id, {
					hard_unavailable_slots: local
				}));
			}

			await Promise.all(ops);
			toast.success('บันทึกการตั้งค่าสำเร็จ');
			priorityDirty = false;
			await loadAll();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function slotsEqual(a: TimeSlot[], b: TimeSlot[]): boolean {
		if (a.length !== b.length) return false;
		const setA = new Set(a.map((s) => slotKey(s.day, s.period_id)));
		for (const s of b) if (!setA.has(slotKey(s.day, s.period_id))) return false;
		return true;
	}

	function toggleExpand(id: string) {
		const next = new Set(expandedIds);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expandedIds = next;
	}

	function unavailableCount(id: string): number {
		return unavailableEdits.get(id)?.length ?? 0;
	}

	onMount(loadAll);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<div class="container mx-auto p-4 space-y-4">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Sparkles class="w-6 h-6 text-primary" />
			<h1 class="text-2xl font-bold">ตั้งค่าจัดตารางอัตโนมัติ</h1>
		</div>
		<Button onclick={saveAll} disabled={saving || loading}>
			{#if saving}
				<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
			{:else}
				<Save class="w-4 h-4 mr-2" />
			{/if}
			บันทึก
		</Button>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<LoaderCircle class="w-8 h-8 animate-spin text-muted-foreground" />
		</div>
	{:else}
		<!-- Global settings -->
		<Card.Root class="p-4">
			<h2 class="font-semibold mb-3">ตั้งค่ารวม</h2>
			<div class="flex items-center gap-3">
				<Label for="max-consec" class="shrink-0">ครูสอนติดสูงสุด:</Label>
				<Input
					id="max-consec"
					type="number"
					min="1"
					max="20"
					bind:value={defaultMaxConsecutive}
					class="w-24"
				/>
				<span class="text-sm text-muted-foreground">คาบติด (default 4)</span>
			</div>
		</Card.Root>

		<!-- Instructor priority + constraints -->
		<Card.Root class="p-4">
			<div class="mb-3">
				<h2 class="font-semibold">ลำดับครู (ลากเพื่อจัดเรียง)</h2>
				<p class="text-sm text-muted-foreground">
					ครูที่อยู่บนสุด จะถูกจัดตารางก่อน — แสดงเฉพาะครูที่เป็น primary ของวิชา
					({instructors.length} คน)
				</p>
			</div>

			{#if instructors.length === 0}
				<p class="text-muted-foreground text-center py-8">
					ยังไม่มีครูที่เป็น primary instructor — เพิ่มได้ที่หน้า Course Planning
				</p>
			{:else}
				<div class="space-y-2">
					{#each instructors as instr, idx (instr.id)}
						<div
							draggable="true"
							ondragstart={(e) => onDragStart(e, instr.id)}
							ondragover={onDragOver}
							ondragenter={(e) => onDragEnter(e, instr.id)}
							ondragend={onDragEnd}
							role="listitem"
							class="border rounded-md bg-card transition-shadow {draggedId === instr.id ? 'opacity-40' : ''}"
						>
							<!-- Header row -->
							<div class="flex items-center gap-2 p-2">
								<GripVertical class="w-4 h-4 text-muted-foreground cursor-move shrink-0" />
								<Badge variant="secondary" class="shrink-0 w-10 justify-center">
									{idx + 1}
								</Badge>
								<button
									onclick={() => toggleExpand(instr.id)}
									class="flex items-center gap-2 flex-1 text-left hover:bg-accent rounded px-2 py-1"
								>
									{#if expandedIds.has(instr.id)}
										<ChevronDown class="w-4 h-4" />
									{:else}
										<ChevronRight class="w-4 h-4" />
									{/if}
									<span class="font-medium">{instr.first_name} {instr.last_name}</span>
									<span class="text-xs text-muted-foreground">
										({instr.primary_course_count} วิชา)
									</span>
									{#if unavailableCount(instr.id) > 0}
										<Badge variant="outline" class="ml-auto text-xs">
											ไม่ว่าง {unavailableCount(instr.id)} คาบ
										</Badge>
									{/if}
								</button>
							</div>

							<!-- Expanded content -->
							{#if expandedIds.has(instr.id)}
								<div class="border-t p-3 bg-muted/30 space-y-3">
									<div>
										<h4 class="text-sm font-medium mb-2">คาบที่ไม่ว่าง</h4>
										<div class="overflow-x-auto">
											<table class="text-xs border-collapse">
												<thead>
													<tr>
														<th class="border p-1 bg-card sticky left-0 z-10">วัน</th>
														{#each periods as p (p.id)}
															<th class="border p-1 bg-card min-w-[60px]">
																{p.name || `คาบ ${p.order_index}`}
															</th>
														{/each}
													</tr>
												</thead>
												<tbody>
													{#each schoolDays as day (day.value)}
														<tr>
															<td class="border p-1 bg-card font-medium sticky left-0 z-10">
																{day.shortLabel}
															</td>
															{#each periods as p (p.id)}
																<td class="border p-0 text-center">
																	<button
																		onclick={() => toggleUnavailable(instr.id, day.value, p.id)}
																		class="w-full h-7 hover:bg-accent transition-colors {isUnavailable(
																			instr.id,
																			day.value,
																			p.id
																		)
																			? 'bg-destructive/80 hover:bg-destructive text-destructive-foreground'
																			: ''}"
																		aria-label={isUnavailable(instr.id, day.value, p.id)
																			? 'คลิกเพื่อตั้งเป็นว่าง'
																			: 'คลิกเพื่อตั้งเป็นไม่ว่าง'}
																	>
																		{isUnavailable(instr.id, day.value, p.id) ? '✕' : ''}
																	</button>
																</td>
															{/each}
														</tr>
													{/each}
												</tbody>
											</table>
										</div>
										<p class="text-xs text-muted-foreground mt-1">
											คลิกเพื่อ toggle "ไม่ว่าง" — คาบสีแดง = ครูจะไม่ถูกจัดในคาบนั้น
										</p>
									</div>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</Card.Root>
	{/if}
</div>
