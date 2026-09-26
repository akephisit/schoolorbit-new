<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { untrack } from 'svelte';
	import {
		createHomeroomPlacement,
		createStudentAcademicYear,
		listGradeLevelOptions,
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
	import { SvelteMap } from 'svelte/reactivity';
	import { STUDENT_YEARS_WORKSPACE_DEPENDENCY } from '$lib/academic-core/foundation-route';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import StudentYearPlacementEditor from '$lib/components/academic-core/StudentYearPlacementEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Plus, Search, UserRoundSearch } from '@lucide/svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const academicYearId = $derived(data.academicYearId);
	let studentYears = $state<StudentAcademicYear[]>([]);
	const placementsByStudentYear = new SvelteMap<string, HomeroomPlacement[]>();
	let homerooms = $state<Homeroom[]>([]);
	let gradeLevelOptions = $state<GradeLevelOption[]>([]);
	let programOptions = $state<StudyProgramOption[]>([]);
	let loading = $state(true);
	let hasWorkspace = $state(false);
	let errorMessage = $state('');
	const candidateRequest = new LatestRequest();
	const createOptionsRequest = new LatestRequest();
	let loadedYearId: string | null = null;
	let mutationRevision = 0;
	const canManage = $derived($can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL));

	let createDialogOpen = $state(false);
	let createBusy = $state(false);
	let createError = $state('');
	let candidateSearch = $state('');
	let candidates = $state<StudentYearCandidate[]>([]);
	let candidatesLoading = $state(false);
	let createOptionsLoading = $state(false);
	let createDraft = $state({ studentId: '', gradeLevelId: '', studyProgramId: '' });

	let detailDialogOpen = $state(false);
	let selectedStudentYear = $state<StudentAcademicYear | null>(null);

	function retryWorkspace() {
		return invalidate(STUDENT_YEARS_WORKSPACE_DEPENDENCY);
	}

	$effect.pre(() => {
		const result = data.workspace;
		const yearId = data.academicYearId;
		const initialMutationRevision = mutationRevision;
		let current = true;
		untrack(() => {
			if (loadedYearId !== yearId) {
				loadedYearId = yearId;
				studentYears = [];
				placementsByStudentYear.clear();
				homerooms = [];
				gradeLevelOptions = [];
				programOptions = [];
				candidates = [];
				createDialogOpen = false;
				detailDialogOpen = false;
				selectedStudentYear = null;
				hasWorkspace = false;
				candidateRequest.abort();
				createOptionsRequest.abort();
			}
			loading = Boolean(result);
			errorMessage = '';
		});
		if (result) {
			void result.then((outcome) => {
				if (!current) return;
				untrack(() => {
					if (outcome.ok && mutationRevision === initialMutationRevision) {
						studentYears = outcome.data.studentYears;
						placementsByStudentYear.clear();
						for (const [recordId, placements] of outcome.data.placementsByStudentYearId)
							placementsByStudentYear.set(recordId, placements);
						homerooms = outcome.data.homerooms;
						hasWorkspace = true;
					} else if (!outcome.ok) errorMessage = outcome.error;
					loading = false;
				});
			});
		}
		return () => {
			current = false;
		};
	});

	async function loadCreateOptions() {
		if (!academicYearId || !canManage) return;
		const { revision, signal } = createOptionsRequest.begin();
		createOptionsLoading = true;
		try {
			const [grades, programs] = await Promise.all([
				listGradeLevelOptions(academicYearId, { signal }),
				listStudyProgramOptionsForAcademicYear(academicYearId, { signal })
			]);
			if (!createOptionsRequest.isCurrent(revision)) return;
			gradeLevelOptions = grades;
			programOptions = programs;
		} catch (error) {
			if (isAbortError(error)) return;
			if (createOptionsRequest.isCurrent(revision))
				createError =
					error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับเพิ่มนักเรียนไม่สำเร็จ';
		} finally {
			if (createOptionsRequest.isCurrent(revision)) createOptionsLoading = false;
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
		if (!canManage) return;
		createDraft = { studentId: '', gradeLevelId: '', studyProgramId: '' };
		candidateSearch = '';
		candidates = [];
		createError = '';
		createDialogOpen = true;
		void loadCandidates();
		void loadCreateOptions();
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
			mutationRevision += 1;
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
		mutationRevision += 1;
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
		mutationRevision += 1;
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
	{:else if loading && !hasWorkspace}
		<PageSkeleton variant="table" rows={8} />
	{:else if errorMessage && !hasWorkspace}
		<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={retryWorkspace}
		/>
	{:else}
		<div class="relative space-y-4" aria-busy={loading} data-testid="student-years-ready">
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตนักเรียนประจำปี" />{/if}
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
			{#if errorMessage && hasWorkspace}
				<div role="alert" class="flex items-center gap-2 text-sm text-destructive">
					<span>{errorMessage}</span>
					<Button size="sm" variant="outline" onclick={retryWorkspace}>ลองอีกครั้ง</Button>
				</div>
			{/if}
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
								(createOptionsLoading ? 'กำลังโหลดระดับชั้น…' : 'เลือกระดับชั้น')}</Select.Trigger
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
								: createOptionsLoading
									? 'กำลังโหลดแผนการเรียน…'
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
				><Button type="submit" disabled={createBusy || candidatesLoading || createOptionsLoading}
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
