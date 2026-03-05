<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listScoresByPeriod,
		listExamSubjects,
		createExamSubject,
		updateExamSubject,
		deleteExamSubject,
		batchUpsertScores,
		type AdmissionExamSubject,
		type ScoreRow
	} from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import Plus from 'lucide-svelte/icons/plus';
	import Trash2 from 'lucide-svelte/icons/trash-2';
	import Pencil from 'lucide-svelte/icons/pencil';
	import Save from 'lucide-svelte/icons/save';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import ArrowUpDown from 'lucide-svelte/icons/arrow-up-down';
	import Calculator from 'lucide-svelte/icons/calculator';

	let { data } = $props();
	const { periodId } = data;

	let subjects = $state<AdmissionExamSubject[]>([]);
	let rows = $state<ScoreRow[]>([]);
	let loading = $state(true);
	let saving = $state(false);

	// เลือกวิชาที่ใช้ sort/คำนวณ
	let selectedSubjectIds = $state<Set<string>>(new Set());
	let sortDir = $state<'asc' | 'desc'>('desc');

	// เก็บ draft scores (ก่อน save)
	let draftScores = $state<Map<string, Map<string, number>>>(new Map()); // appId → subjectId → score

	// Dialog เพิ่มวิชา
	let showAddSubjectDialog = $state(false);
	let newSubjectName = $state('');
	let newSubjectCode = $state('');
	let newSubjectMax = $state('100');
	let addingSubject = $state(false);

	// Sort order ปัจจุบัน
	let sortedRows = $derived.by(() => {
		const selected = [...selectedSubjectIds];
		return [...rows].sort((a, b) => {
			const scoreA =
				selected.length > 0
					? selected.reduce((sum, sid) => sum + (a.score_map[sid] ?? 0), 0)
					: a.computed_total;
			const scoreB =
				selected.length > 0
					? selected.reduce((sum, sid) => sum + (b.score_map[sid] ?? 0), 0)
					: b.computed_total;
			return sortDir === 'desc' ? scoreB - scoreA : scoreA - scoreB;
		});
	});

	function getDisplayScore(row: ScoreRow): number {
		const selected = [...selectedSubjectIds];
		if (selected.length === 0) return row.computed_total;
		return selected.reduce((sum, sid) => sum + (row.score_map[sid] ?? 0), 0);
	}

	function getDraftScore(appId: string, subjectId: string): string {
		return (draftScores.get(appId)?.get(subjectId) ?? '').toString();
	}

	function setDraftScore(appId: string, subjectId: string, value: string) {
		if (!draftScores.has(appId)) draftScores.set(appId, new Map());
		const num = parseFloat(value);
		if (!isNaN(num)) {
			draftScores.get(appId)!.set(subjectId, num);
		} else {
			draftScores.get(appId)!.delete(subjectId);
		}
		draftScores = new Map(draftScores);

		// update local score_map for instant feedback
		rows = rows.map((r) => {
			if (r.app_id === appId) {
				const newMap = { ...r.score_map, [subjectId]: num || 0 };
				const total = subjects.reduce((sum, s) => sum + (newMap[s.id] ?? 0), 0);
				return { ...r, score_map: newMap, computed_total: total };
			}
			return r;
		});
	}

	async function saveAllScores() {
		const entries: { application_id: string; exam_subject_id: string; score: number }[] = [];
		for (const [appId, subMap] of draftScores) {
			for (const [subjectId, score] of subMap) {
				entries.push({ application_id: appId, exam_subject_id: subjectId, score });
			}
		}
		if (entries.length === 0) {
			toast.info('ไม่มีการเปลี่ยนแปลง');
			return;
		}
		saving = true;
		try {
			const selectedIds = [...selectedSubjectIds];
			await batchUpsertScores({
				scores: entries,
				recalculate_total: true,
				total_subject_ids: selectedIds.length > 0 ? selectedIds : undefined
			});
			toast.success(`บันทึกคะแนน ${entries.length} รายการเรียบร้อยแล้ว`);
			draftScores = new Map();
			await loadData();
		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function handleAddSubject() {
		if (!newSubjectName.trim()) {
			toast.error('กรุณาใส่ชื่อวิชา');
			return;
		}
		addingSubject = true;
		try {
			await createExamSubject(periodId, {
				subject_name: newSubjectName.trim(),
				subject_code: newSubjectCode.trim() || undefined,
				max_score: parseFloat(newSubjectMax) || 100,
				display_order: subjects.length
			});
			toast.success('เพิ่มวิชาเรียบร้อยแล้ว');
			showAddSubjectDialog = false;
			newSubjectName = '';
			newSubjectCode = '';
			newSubjectMax = '100';
			await loadData();
		} catch (e: any) {
			toast.error(e.message || 'เพิ่มวิชาไม่สำเร็จ');
		} finally {
			addingSubject = false;
		}
	}

	async function handleDeleteSubject(id: string, name: string) {
		if (!confirm(`ลบวิชา "${name}" และคะแนนทั้งหมดใช่ไหม?`)) return;
		try {
			await deleteExamSubject(periodId, id);
			toast.success('ลบวิชาเรียบร้อยแล้ว');
			selectedSubjectIds.delete(id);
			selectedSubjectIds = new Set(selectedSubjectIds);
			await loadData();
		} catch (e: any) {
			toast.error(e.message || 'ลบไม่สำเร็จ');
		}
	}

	async function loadData() {
		try {
			const res = await listScoresByPeriod(periodId);
			subjects = res.subjects;
			rows = res.applications;
			// init selectedSubjectIds ด้วยทุกวิชา
			if (selectedSubjectIds.size === 0 && subjects.length > 0) {
				selectedSubjectIds = new Set(subjects.map((s) => s.id));
			}
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	function toggleSubject(id: string) {
		if (selectedSubjectIds.has(id)) selectedSubjectIds.delete(id);
		else selectedSubjectIds.add(id);
		selectedSubjectIds = new Set(selectedSubjectIds);
	}

	const hasDraft = $derived(draftScores.size > 0);

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between gap-4">
		<div class="flex items-center gap-3">
			<Button variant="ghost" size="icon" href="/staff/academic/admission/{periodId}">
				<ArrowLeft class="h-4 w-4" />
			</Button>
			<div>
				<h1 class="flex items-center gap-2 text-xl font-bold">
					<Calculator class="h-5 w-5 text-primary" />
					จัดการคะแนนสอบ
				</h1>
				<p class="text-sm text-muted-foreground">{rows.length} ใบสมัคร | {subjects.length} วิชา</p>
			</div>
		</div>
		<div class="flex gap-2">
			<Button variant="outline" size="sm" onclick={() => (showAddSubjectDialog = true)}>
				<Plus class="mr-1.5 h-3.5 w-3.5" />วิชาใหม่
			</Button>
			{#if hasDraft}
				<Button size="sm" onclick={saveAllScores} disabled={saving}>
					{#if saving}<Loader2 class="mr-1.5 h-3.5 w-3.5 animate-spin" />{:else}<Save
							class="mr-1.5 h-3.5 w-3.5"
						/>{/if}
					บันทึกทั้งหมด
				</Button>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="flex h-48 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else if subjects.length === 0}
		<Card.Root>
			<Card.Content class="flex flex-col items-center justify-center py-16 text-center">
				<Calculator class="mb-4 h-12 w-12 text-muted-foreground/50" />
				<p class="text-lg font-medium">ยังไม่มีวิชาสอบ</p>
				<p class="text-sm text-muted-foreground mt-1">เพิ่มวิชาสอบก่อนเพื่อเริ่มกรอกคะแนน</p>
				<Button class="mt-4" onclick={() => (showAddSubjectDialog = true)}>
					<Plus class="mr-2 h-4 w-4" />เพิ่มวิชาสอบ
				</Button>
			</Card.Content>
		</Card.Root>
	{:else}
		<!-- Subject Filter + Sort config -->
		<Card.Root>
			<Card.Header class="pb-3">
				<div class="flex items-center justify-between">
					<Card.Title class="text-base">ตั้งค่าการเรียงและวิชาที่ใช้คำนวณ</Card.Title>
					<Button
						variant="ghost"
						size="sm"
						onclick={() => (sortDir = sortDir === 'desc' ? 'asc' : 'desc')}
					>
						<ArrowUpDown class="mr-1.5 h-3.5 w-3.5" />
						{sortDir === 'desc' ? 'มากไปน้อย' : 'น้อยไปมาก'}
					</Button>
				</div>
			</Card.Header>
			<Card.Content>
				<div class="flex flex-wrap gap-3">
					{#each subjects as subject}
						<label
							class="flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-sm
							{selectedSubjectIds.has(subject.id) ? 'border-primary bg-primary/5' : 'border-border'}"
						>
							<Checkbox
								checked={selectedSubjectIds.has(subject.id)}
								onCheckedChange={() => toggleSubject(subject.id)}
							/>
							<span class="font-medium">{subject.subject_name}</span>
							<span class="text-muted-foreground">({subject.max_score} คะแนน)</span>
							<button
								class="ml-1 text-red-400 hover:text-red-600"
								onclick={() => handleDeleteSubject(subject.id, subject.subject_name)}
							>
								<Trash2 class="h-3.5 w-3.5" />
							</button>
						</label>
					{/each}
					<p class="w-full text-xs text-muted-foreground mt-1">
						✓ = วิชาที่เลือกจะใช้ในการเรียงลำดับและคำนวณคะแนนรวม
					</p>
				</div>
			</Card.Content>
		</Card.Root>

		<!-- Score Table -->
		<Card.Root>
			<Card.Content class="p-0">
				<div class="overflow-x-auto">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b bg-muted/40">
								<th class="sticky left-0 bg-muted/40 px-4 py-3 text-left">#</th>
								<th class="sticky left-8 min-w-[180px] bg-muted/40 px-4 py-3 text-left"
									>ชื่อผู้สมัคร</th
								>
								<th class="px-4 py-3 text-left text-muted-foreground">ระดับ</th>
								{#each subjects as subject}
									<th
										class="min-w-[100px] px-4 py-3 text-center
										{selectedSubjectIds.has(subject.id) ? 'text-primary font-semibold' : 'text-muted-foreground'}"
									>
										{subject.subject_name}
										<div class="text-xs font-normal opacity-70">/{subject.max_score}</div>
									</th>
								{/each}
								<th class="min-w-[90px] bg-primary/5 px-4 py-3 text-center font-bold text-primary">
									รวม{selectedSubjectIds.size < subjects.length ? ' *' : ''}
								</th>
							</tr>
						</thead>
						<tbody>
							{#each sortedRows as row, i}
								<tr
									class="border-b transition-colors hover:bg-muted/20
									{draftScores.has(row.app_id) ? 'bg-amber-50' : ''}"
								>
									<td class="sticky left-0 bg-background px-4 py-2 text-muted-foreground"
										>{i + 1}</td
									>
									<td class="sticky min-w-[180px] bg-background px-4 py-2 font-medium">
										<div>{row.name}</div>
										<div class="text-xs text-muted-foreground">{row.application_number}</div>
									</td>
									<td class="px-4 py-2 text-muted-foreground">{row.grade_level_name ?? '-'}</td>
									{#each subjects as subject}
										<td class="px-2 py-2 text-center">
											<Input
												type="number"
												class="h-8 w-20 text-center text-sm"
												value={getDraftScore(row.app_id, subject.id) ||
													(row.score_map[subject.id] ?? '').toString()}
												min="0"
												max={subject.max_score}
												step="0.5"
												oninput={(e) =>
													setDraftScore(
														row.app_id,
														subject.id,
														(e.target as HTMLInputElement).value
													)}
											/>
										</td>
									{/each}
									<td class="bg-primary/5 px-4 py-2 text-center font-bold text-primary">
										{getDisplayScore(row).toFixed(1)}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
					{#if rows.length === 0}
						<div class="py-12 text-center text-muted-foreground">
							ยังไม่มีใบสมัครที่ผ่านการพิจารณา
						</div>
					{/if}
				</div>
				{#if hasDraft}
					<div class="border-t bg-amber-50 px-4 py-3 text-sm text-amber-700">
						⚠️ มีการเปลี่ยนแปลงที่ยังไม่ได้บันทึก — กด "บันทึกทั้งหมด" ด้านบนซ้าย
					</div>
				{/if}
			</Card.Content>
		</Card.Root>
	{/if}
</div>

<!-- Add Subject Dialog -->
<Dialog.Root bind:open={showAddSubjectDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title>เพิ่มวิชาสอบ</Dialog.Title>
		</Dialog.Header>
		<div class="grid gap-4 py-2">
			<div class="grid gap-2">
				<Label>ชื่อวิชา <span class="text-red-500">*</span></Label>
				<Input bind:value={newSubjectName} placeholder="เช่น คณิตศาสตร์" />
			</div>
			<div class="grid gap-2">
				<Label>รหัสวิชา</Label>
				<Input bind:value={newSubjectCode} placeholder="เช่น MATH" />
			</div>
			<div class="grid gap-2">
				<Label>คะแนนเต็ม</Label>
				<Input type="number" bind:value={newSubjectMax} min="1" max="1000" />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showAddSubjectDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleAddSubject} disabled={addingSubject}>
				{#if addingSubject}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				เพิ่มวิชา
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
