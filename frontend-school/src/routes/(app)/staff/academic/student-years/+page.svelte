<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createHomeroomPlacement,
		createStudentAcademicYear,
		listGradeLevelOptions,
		listHomeroomPlacements,
		listHomerooms,
		listStudentAcademicYears,
		listStudentOptions,
		listStudyProgramOptionsForYear,
		transferHomeroomPlacement,
		type Homeroom,
		type HomeroomPlacement,
		type StudentAcademicYear
	} from '$lib/api/academic-core';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import StudentYearPlacementEditor from '$lib/components/academic-core/StudentYearPlacementEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
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
	let revision = 0;
	const canManage = $derived($can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL));

	async function loadWorkspace(yearId: string) {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const records = await listStudentAcademicYears(yearId);
			const nextPlacements = new SvelteMap<string, HomeroomPlacement[]>();
			for (const record of records)
				nextPlacements.set(record.id, await listHomeroomPlacements(record.id));
			const rooms = await listHomerooms(yearId);
			const students = await listStudentOptions();
			const levels = await listGradeLevelOptions(yearId);
			const programs = await listStudyProgramOptionsForYear(yearId);
			if (current !== revision) return;
			studentYears = records;
			placementsByStudentYear.clear();
			for (const [recordId, placements] of nextPlacements)
				placementsByStudentYear.set(recordId, placements);
			homerooms = rooms;
			studentOptions = students.map((student) => ({ id: student.id, name: student.name }));
			gradeLevelOptions = levels.map((level) => ({ id: level.id, name: level.name }));
			programOptions = programs.map((program) => ({
				id: program.id,
				name: `${program.curriculumName} · ${program.name}`
			}));
		} catch (error) {
			if (current === revision)
				errorMessage =
					error instanceof Error ? error.message : 'โหลดข้อมูลนักเรียนประจำปีไม่สำเร็จ';
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function addStudentYear(event: SubmitEvent) {
		event.preventDefault();
		if (!academicYearId) return;
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
		return academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			if (yearId && yearId !== loadedYearId) {
				loadedYearId = yearId;
				void loadWorkspace(yearId);
			}
		});
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
				<label class="space-y-1.5 text-sm"
					><span class="font-medium">นักเรียน</span><select
						class="h-10 w-full rounded-md border bg-background px-3"
						bind:value={draft.studentId}
						required
						><option value="">เลือกนักเรียน</option
						>{#each studentOptions as option (option.id)}<option value={option.id}
								>{option.name}</option
							>{/each}</select
					></label
				><label class="space-y-1.5 text-sm"
					><span class="font-medium">ระดับชั้น</span><select
						class="h-10 w-full rounded-md border bg-background px-3"
						bind:value={draft.gradeLevelId}
						required
						><option value="">เลือกระดับชั้น</option
						>{#each gradeLevelOptions as option (option.id)}<option value={option.id}
								>{option.name}</option
							>{/each}</select
					></label
				><label class="space-y-1.5 text-sm"
					><span class="font-medium">แผนการเรียน</span><select
						class="h-10 w-full rounded-md border bg-background px-3"
						bind:value={draft.studyProgramId}
						required
						><option value="">เลือกแผน</option>{#each programOptions as option (option.id)}<option
								value={option.id}>{option.name}</option
							>{/each}</select
					></label
				><Button type="submit"><Plus class="size-4" /> เพิ่มในปีนี้</Button>
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
