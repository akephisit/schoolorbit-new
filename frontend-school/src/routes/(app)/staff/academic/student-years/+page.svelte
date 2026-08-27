<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import {
		createHomeroomPlacement,
		createStudentAcademicYear,
		listGradeLevelOptions,
		listHomerooms,
		listPlacementsForAcademicYear,
		listStudentAcademicYears,
		listStudentOptions,
		listStudyProgramOptionsForAcademicYear,
		transferHomeroomPlacement,
		type Homeroom,
		type HomeroomPlacement,
		type StudentAcademicYear
	} from '$lib/api/academic-core';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import StudentYearPlacementEditor from '$lib/components/academic-core/StudentYearPlacementEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { loadStudentYearCollections } from '$lib/workspaces/academic-batch';
	import { Plus } from 'lucide-svelte';

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let studentYears = $state<StudentAcademicYear[]>([]);
	const placementsByStudentYear = new SvelteMap<string, HomeroomPlacement[]>();
	let homerooms = $state<Homeroom[]>([]);
	let studentOptions = $state<Array<{ id: string; name: string }>>([]);
	let gradeLevelOptions = $state<Array<{ id: string; name: string }>>([]);
	let programOptions = $state<Array<{ id: string; name: string }>>([]);
	let draft = $state({ studentId: '', gradeLevelId: '', studyProgramId: '' });
	let loading = $state(false);
	let errorMessage = $state('');
	const request = new LatestRequest();
	const canManage = $derived($can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL));

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
					listStudentOptions: (search, options) => listStudentOptions(yearId, search, options),
					listGradeLevelOptions,
					listStudyProgramOptionsForAcademicYear
				},
				yearId,
				signal
			);
			if (!request.isCurrent(revision)) return;
			studentYears = workspace.studentYears;
			placementsByStudentYear.clear();
			for (const [recordId, placements] of workspace.placementsByStudentYearId)
				placementsByStudentYear.set(recordId, placements);
			homerooms = workspace.homerooms;
			studentOptions = workspace.students.map((student) => ({
				id: student.id,
				name: student.name
			}));
			gradeLevelOptions = workspace.gradeLevels.map((level) => ({
				id: level.id,
				name: level.name
			}));
			programOptions = workspace.programs.map((program) => ({
				id: program.id,
				name: `${program.curriculumName} · ${program.name}`
			}));
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision))
				errorMessage =
					error instanceof Error ? error.message : 'โหลดข้อมูลนักเรียนประจำปีไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function addStudentYear(event: SubmitEvent) {
		event.preventDefault();
		if (!academicYearId || !draft.studentId || !draft.gradeLevelId || !draft.studyProgramId) {
			toast.error('กรุณาเลือกนักเรียน ระดับชั้น และแผนการเรียนให้ครบ');
			return;
		}
		const created = await createStudentAcademicYear({ academicYearId, ...draft });
		studentYears = [...studentYears, created];
		placementsByStudentYear.set(created.id, []);
		draft = { studentId: '', gradeLevelId: '', studyProgramId: '' };
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
		await createHomeroomPlacement(record.id, {
			...placementDraft,
			status: record.status === 'planned' ? 'planned' : 'current',
			rowVersion: record.rowVersion
		});
		if (academicYearId) await loadWorkspace(academicYearId);
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
		if (academicYearId) await loadWorkspace(academicYearId);
		return result;
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
			if (yearId && yearId !== loadedYearId) {
				loadedYearId = yearId;
				void loadWorkspace(yearId);
			}
		});
		return () => {
			unsubscribe();
			request.abort();
		};
	});
</script>

<PageShell
	title="นักเรียนประจำปี"
	description="ข้อมูลระดับชั้นและการจัดห้องแยกตามปี พร้อมประวัติการย้ายห้องที่ตรวจสอบย้อนหลังได้"
>
	{#if !academicYearId}<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกบริบทบนแถบด้านบน"
		/>{:else if loading}<PageSkeleton variant="cards" rows={5} />{:else if errorMessage}<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicYearId)}
		/>{:else}
		{#if canManage}<form
				class="grid gap-3 rounded-xl border bg-card p-4 shadow-sm md:grid-cols-[1fr_1fr_1fr_auto] md:items-end"
				onsubmit={addStudentYear}
			>
				<div class="space-y-1.5">
					<Label for="student-year-student">นักเรียน</Label>
					<Select.Root type="single" bind:value={draft.studentId}>
						<Select.Trigger id="student-year-student" class="w-full">
							{studentOptions.find((option) => option.id === draft.studentId)?.name ??
								'เลือกนักเรียน'}
						</Select.Trigger>
						<Select.Content>
							{#each studentOptions as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label for="student-year-grade">ระดับชั้น</Label>
					<Select.Root type="single" bind:value={draft.gradeLevelId}>
						<Select.Trigger id="student-year-grade" class="w-full">
							{gradeLevelOptions.find((option) => option.id === draft.gradeLevelId)?.name ??
								'เลือกระดับชั้น'}
						</Select.Trigger>
						<Select.Content>
							{#each gradeLevelOptions as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label for="student-year-program">แผนการเรียน</Label>
					<Select.Root type="single" bind:value={draft.studyProgramId}>
						<Select.Trigger id="student-year-program" class="w-full">
							{programOptions.find((option) => option.id === draft.studyProgramId)?.name ??
								'เลือกแผน'}
						</Select.Trigger>
						<Select.Content>
							{#each programOptions as option (option.id)}
								<Select.Item value={option.id}>{option.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<Button type="submit"><Plus class="size-4" /> เพิ่มในปีนี้</Button>
			</form>{/if}
		<div class="space-y-4">
			{#each studentYears as record (record.id)}<StudentYearPlacementEditor
					studentYear={record}
					placements={placementsByStudentYear.get(record.id) ?? []}
					{homerooms}
					{canManage}
					onCreatePlacement={(placementDraft) => addPlacement(record, placementDraft)}
					onTransfer={(placement, transferDraft) => transfer(placement, transferDraft)}
				/>{:else}<div
					class="rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
				>
					ยังไม่มีข้อมูลนักเรียนในปีที่เลือก
				</div>{/each}
		</div>
	{/if}
</PageShell>
