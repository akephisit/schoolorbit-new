<script lang="ts">
	import {
		createHomeroomPlacement,
		createStudentAcademicYear,
		listGradeLevelOptions,
		listHomerooms,
		listPlacementsForAcademicYear,
		listStudentAcademicYears,
		listStudentYearCandidates,
		listStudyProgramOptionsForAcademicYear,
		transferHomeroomPlacement,
		type GradeLevelOption,
		type Homeroom,
		type HomeroomPlacement,
		type StudentAcademicYear,
		type StudentYearCandidate,
		type StudyProgramOption
	} from '$lib/api/academic-core';
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import StudentYearPlacementEditor from '$lib/components/academic-core/StudentYearPlacementEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { loadStudentYearCollections } from '$lib/workspaces/academic-batch';
	import { Plus, Search, UserRoundSearch } from 'lucide-svelte';

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let studentYears = $state<StudentAcademicYear[]>([]);
	const placementsByStudentYear = new SvelteMap<string, HomeroomPlacement[]>();
	let homerooms = $state<Homeroom[]>([]);
	let gradeLevelOptions = $state<GradeLevelOption[]>([]);
	let programOptions = $state<StudyProgramOption[]>([]);
	let loading = $state(false);
	let errorMessage = $state('');
	const request = new LatestRequest();
	const candidateRequest = new LatestRequest();
	const canManage = $derived($can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL));

	let createDialogOpen = $state(false);
	let createBusy = $state(false);
	let createError = $state('');
	let candidateSearch = $state('');
	let candidates = $state<StudentYearCandidate[]>([]);
	let candidatesLoading = $state(false);
	let createDraft = $state({ studentId: '', gradeLevelId: '', studyProgramId: '' });

	let detailDialogOpen = $state(false);
	let selectedStudentYear = $state<StudentAcademicYear | null>(null);

	async function loadWorkspace(yearId: string) {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const workspace = await loadStudentYearCollections(
				{
					listStudentAcademicYears: (selectedYearId, options) =>
						listStudentAcademicYears(selectedYearId, {}, options),
					listPlacementsForAcademicYear,
					listHomerooms,
					listGradeLevelOptions,
					listStudyProgramOptionsForAcademicYear
				},
				yearId,
				signal
			);
			if (!request.isCurrent(revision)) return;
			studentYears = workspace.studentYears;
			placementsByStudentYear.clear();
			for (const [recordId, placements] of workspace.placementsByStudentYearId) {
				placementsByStudentYear.set(recordId, placements);
			}
			homerooms = workspace.homerooms;
			gradeLevelOptions = workspace.gradeLevels;
			programOptions = workspace.programs;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage =
					error instanceof Error ? error.message : 'โหลดข้อมูลนักเรียนประจำปีไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function loadCandidates() {
		if (!academicYearId) return;
		const { revision, signal } = candidateRequest.begin();
		candidatesLoading = true;
		createError = '';
		try {
			const result = await listStudentYearCandidates(academicYearId, candidateSearch, { signal });
			if (candidateRequest.isCurrent(revision)) candidates = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (candidateRequest.isCurrent(revision)) {
				createError = error instanceof Error ? error.message : 'ค้นหานักเรียนไม่สำเร็จ';
			}
		} finally {
			if (candidateRequest.isCurrent(revision)) candidatesLoading = false;
		}
	}

	function openCreateDialog() {
		createDraft = { studentId: '', gradeLevelId: '', studyProgramId: '' };
		candidateSearch = '';
		candidates = [];
		createError = '';
		createDialogOpen = true;
		void loadCandidates();
	}

	async function searchCandidates(event: SubmitEvent) {
		event.preventDefault();
		await loadCandidates();
	}

	async function addStudentYear(event: SubmitEvent) {
		event.preventDefault();
		if (
			!academicYearId ||
			!createDraft.studentId ||
			!createDraft.gradeLevelId ||
			!createDraft.studyProgramId
		) {
			createError = 'กรุณาเลือกนักเรียน ระดับชั้น และแผนการเรียนให้ครบ';
			return;
		}
		createBusy = true;
		createError = '';
		try {
			const created = await createStudentAcademicYear({ academicYearId, ...createDraft });
			studentYears = [...studentYears, created].sort((a, b) =>
				a.studentName.localeCompare(b.studentName, 'th')
			);
			placementsByStudentYear.set(created.id, []);
			candidates = candidates.filter((candidate) => candidate.id !== created.studentId);
			createDialogOpen = false;
		} catch (error) {
			createError = error instanceof Error ? error.message : 'เพิ่มนักเรียนในปีนี้ไม่สำเร็จ';
		} finally {
			createBusy = false;
		}
	}

	function activePlacement(record: StudentAcademicYear): HomeroomPlacement | null {
		return (
			(placementsByStudentYear.get(record.id) ?? []).find((placement) =>
				['current', 'planned'].includes(placement.status)
			) ?? null
		);
	}

	function homeroomName(id: string): string | null {
		return homerooms.find((room) => room.id === id)?.name ?? null;
	}

	function openDetail(record: StudentAcademicYear) {
		selectedStudentYear = record;
		detailDialogOpen = true;
	}

	async function addPlacement(
		record: StudentAcademicYear,
		placementDraft: {
			homeroomId: string;
			startDate: string;
			enrollmentType: string;
			classNumber: number | null;
		}
	) {
		const created = await createHomeroomPlacement(record.id, {
			...placementDraft,
			status: record.status === 'planned' ? 'planned' : 'current',
			rowVersion: record.rowVersion
		});
		placementsByStudentYear.set(record.id, [
			...(placementsByStudentYear.get(record.id) ?? []),
			created
		]);
		studentYears = studentYears.map((item) =>
			item.id === record.id ? { ...item, rowVersion: item.rowVersion + 1 } : item
		);
		if (selectedStudentYear?.id === record.id) {
			selectedStudentYear = {
				...selectedStudentYear,
				rowVersion: selectedStudentYear.rowVersion + 1
			};
		}
	}

	async function transfer(
		placement: HomeroomPlacement,
		transferDraft: {
			targetHomeroomId: string;
			transferDate: string;
			enrollmentType: string;
			classNumber: number | null;
			reason: string;
		}
	) {
		const result = await transferHomeroomPlacement(placement.id, {
			...transferDraft,
			rowVersion: placement.rowVersion,
			idempotencyKey: crypto.randomUUID()
		});
		const placements = placementsByStudentYear.get(placement.studentAcademicYearId) ?? [];
		placementsByStudentYear.set(placement.studentAcademicYearId, [
			...placements.map((item) =>
				item.id === result.endedPlacement.id ? result.endedPlacement : item
			),
			result.newPlacement
		]);
		return result;
	}

	function statusLabel(status: StudentAcademicYear['status']): string {
		return {
			planned: 'เตรียมการ',
			active: 'กำลังเรียน',
			completed: 'จบปีแล้ว',
			graduated: 'จบการศึกษา',
			withdrawn: 'พ้นสภาพ'
		}[status];
	}

	onMount(() => {
		let loadedYearId: string | null = null;
		const unsubscribe = academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			if (!yearId) {
				loadedYearId = null;
				request.abort();
				return;
			}
			if (yearId !== loadedYearId) {
				loadedYearId = yearId;
				void loadWorkspace(yearId);
			}
		});
		return () => {
			unsubscribe();
			request.abort();
			candidateRequest.abort();
		};
	});
</script>

<PageShell
	title="นักเรียนประจำปี"
	description="ข้อมูลระดับชั้นและการจัดห้องแยกตามปี พร้อมประวัติการย้ายห้องที่ตรวจสอบย้อนหลังได้"
>
	{#if !academicYearId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกปีการศึกษาบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={8} />
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicYearId)}
		/>
	{:else}
		<div class="space-y-4">
			<div
				class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3 sm:p-4"
			>
				<div>
					<p class="text-sm font-semibold">{studentYears.length} คนในปีที่เลือก</p>
					<p class="text-xs text-muted-foreground">หนึ่งคนมีข้อมูลประจำปีได้เพียงหนึ่งรายการ</p>
				</div>
				{#if canManage}<Button type="button" onclick={openCreateDialog}
						><Plus class="size-4" /> เพิ่มนักเรียนในปีนี้</Button
					>{/if}
			</div>

			<div class="overflow-x-auto rounded-xl border bg-card">
				<Table.Root>
					<Table.Header
						><Table.Row
							><Table.Head class="min-w-32 ps-5">รหัสนักเรียน</Table.Head><Table.Head
								class="min-w-52">ชื่อ–นามสกุล</Table.Head
							><Table.Head class="min-w-36">ระดับชั้น</Table.Head><Table.Head class="min-w-56"
								>แผนการเรียน</Table.Head
							><Table.Head class="min-w-40">ห้องปัจจุบัน</Table.Head><Table.Head class="text-center"
								>เลขที่</Table.Head
							><Table.Head>สถานะ</Table.Head><Table.Head class="w-20"
								><span class="sr-only">รายละเอียด</span></Table.Head
							></Table.Row
						></Table.Header
					>
					<Table.Body>
						{#each studentYears as record (record.id)}
							{@const placement = activePlacement(record)}
							<Table.Row>
								<Table.Cell class="border-s-4 border-s-primary ps-5 font-mono text-xs"
									>{record.studentCode ?? 'ยังไม่มีรหัส'}</Table.Cell
								>
								<Table.Cell class="font-medium">{record.studentName}</Table.Cell>
								<Table.Cell>{record.gradeLevelName}</Table.Cell>
								<Table.Cell class="whitespace-normal">{record.studyProgramName}</Table.Cell>
								<Table.Cell>
									{#if !placement}<span class="text-muted-foreground">ยังไม่ได้จัดห้อง</span
										>{:else if homeroomName(placement.homeroomId)}{homeroomName(
											placement.homeroomId
										)}{:else}<span class="text-destructive">ไม่พบห้องประจำชั้นที่อ้างอิง</span>{/if}
								</Table.Cell>
								<Table.Cell class="text-center tabular-nums"
									>{placement?.classNumber ?? '—'}</Table.Cell
								>
								<Table.Cell
									><Badge variant="outline">{statusLabel(record.status)}</Badge></Table.Cell
								>
								<Table.Cell
									><Button
										type="button"
										size="sm"
										variant="ghost"
										onclick={() => openDetail(record)}>ดูรายละเอียด</Button
									></Table.Cell
								>
							</Table.Row>
						{:else}
							<Table.Row
								><Table.Cell colspan={8} class="h-32 text-center text-muted-foreground"
									>ยังไม่มีนักเรียนในปีที่เลือก</Table.Cell
								></Table.Row
							>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={createDialogOpen}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header
			><Dialog.Title>เพิ่มนักเรียนในปีนี้</Dialog.Title><Dialog.Description
				>ค้นหาเฉพาะนักเรียนที่ยังไม่มีข้อมูลในปีการศึกษาที่เลือก แล้วกำหนดระดับชั้นกับแผนการเรียน</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-3 rounded-xl border bg-muted/20 p-3" onsubmit={searchCandidates}>
			<Label for="student-candidate-search">ค้นหาด้วยชื่อหรือรหัสนักเรียน</Label>
			<div class="flex gap-2">
				<Input
					id="student-candidate-search"
					bind:value={candidateSearch}
					placeholder="พิมพ์ชื่อหรือรหัสนักเรียน"
				/><Button type="submit" variant="outline" disabled={candidatesLoading}
					><Search class="size-4" /> ค้นหา</Button
				>
			</div>
		</form>
		<form class="space-y-4" onsubmit={addStudentYear}>
			<div class="space-y-1.5">
				<Label for="student-year-student">นักเรียน</Label>
				<Select.Root type="single" bind:value={createDraft.studentId}>
					<Select.Trigger id="student-year-student" class="w-full"
						>{candidates.find((candidate) => candidate.id === createDraft.studentId)?.name ??
							(candidatesLoading ? 'กำลังค้นหา…' : 'เลือกนักเรียน')}</Select.Trigger
					>
					<Select.Content
						>{#each candidates as candidate (candidate.id)}<Select.Item value={candidate.id}
								>{candidate.studentCode
									? `${candidate.studentCode} · `
									: ''}{candidate.name}</Select.Item
							>{/each}</Select.Content
					>
				</Select.Root>
				{#if !candidatesLoading && candidates.length === 0}<p
						class="flex items-center gap-2 text-xs text-muted-foreground"
					>
						<UserRoundSearch class="size-4" /> ไม่พบนักเรียนที่ยังเพิ่มได้ ลองเปลี่ยนคำค้นหา
					</p>{/if}
			</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-1.5">
					<Label for="student-year-grade">ระดับชั้น</Label><Select.Root
						type="single"
						bind:value={createDraft.gradeLevelId}
						><Select.Trigger id="student-year-grade" class="w-full"
							>{gradeLevelOptions.find((option) => option.id === createDraft.gradeLevelId)?.name ??
								'เลือกระดับชั้น'}</Select.Trigger
						><Select.Content
							>{#each gradeLevelOptions as option (option.id)}<Select.Item value={option.id}
									>{option.name}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
				</div>
				<div class="space-y-1.5">
					<Label for="student-year-program">แผนการเรียน</Label><Select.Root
						type="single"
						bind:value={createDraft.studyProgramId}
						><Select.Trigger id="student-year-program" class="w-full"
							>{@const program = programOptions.find(
								(option) => option.id === createDraft.studyProgramId
							)}{program
								? `${program.curriculumName} · ${program.name}`
								: 'เลือกแผนการเรียน'}</Select.Trigger
						><Select.Content
							>{#each programOptions as option (option.id)}<Select.Item value={option.id}
									>{option.curriculumName} · {option.name}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					>
				</div>
			</div>
			{#if createError}<p role="alert" class="text-sm text-destructive">{createError}</p>{/if}
			<Dialog.Footer
				><Button type="submit" disabled={createBusy || candidatesLoading}
					><Plus class="size-4" /> เพิ่มนักเรียนในปีนี้</Button
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={detailDialogOpen}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
		{#if selectedStudentYear}
			<Dialog.Header
				><Dialog.Title>{selectedStudentYear.studentName}</Dialog.Title><Dialog.Description
					>{selectedStudentYear.studentCode ?? 'ยังไม่มีรหัสนักเรียน'} · {selectedStudentYear.gradeLevelName}
					· {selectedStudentYear.studyProgramName}</Dialog.Description
				></Dialog.Header
			>
			<StudentYearPlacementEditor
				studentYear={selectedStudentYear}
				placements={placementsByStudentYear.get(selectedStudentYear.id) ?? []}
				{homerooms}
				{canManage}
				onCreatePlacement={(placementDraft) => addPlacement(selectedStudentYear!, placementDraft)}
				onTransfer={(placement, transferDraft) => transfer(placement, transferDraft)}
			/>
		{/if}
	</Dialog.Content>
</Dialog.Root>
