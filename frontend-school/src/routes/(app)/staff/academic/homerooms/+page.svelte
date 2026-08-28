<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createHomeroom,
		getHomeroom,
		listGradeLevelOptions,
		listHomeroomAdvisorsForAcademicYear,
		listHomerooms,
		listStaffOptions,
		listStudyProgramOptionsForAcademicYear,
		replaceHomeroomAdvisors,
		updateHomeroom,
		type CreateHomeroomRequest,
		type GradeLevelOption,
		type Homeroom,
		type HomeroomAdvisor,
		type ReplaceHomeroomAdvisorsRequest,
		type StudyProgramOption,
		type UpdateHomeroomRequest
	} from '$lib/api/academic-core';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import HomeroomEditor from '$lib/components/academic-core/HomeroomEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { loadHomeroomCollections } from '$lib/workspaces/academic-batch';

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let homerooms = $state<Homeroom[]>([]);
	let gradeLevelOptions = $state<GradeLevelOption[]>([]);
	let programOptions = $state<StudyProgramOption[]>([]);
	const advisorsByHomeroom = new SvelteMap<string, HomeroomAdvisor[]>();
	let loading = $state(false);
	let errorMessage = $state('');
	const request = new LatestRequest();
	const canManage = $derived($can.has(PERMISSIONS.HOMEROOM_MANAGE_SCHOOL));

	async function loadWorkspace(yearId: string) {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const workspace = await loadHomeroomCollections(
				{
					listHomerooms,
					listHomeroomAdvisorsForAcademicYear,
					listGradeLevelOptions,
					listStudyProgramOptionsForAcademicYear
				},
				yearId,
				signal
			);
			if (!request.isCurrent(revision)) return;
			homerooms = workspace.homerooms;
			advisorsByHomeroom.clear();
			for (const [roomId, advisors] of workspace.advisorsByHomeroomId)
				advisorsByHomeroom.set(roomId, advisors);
			gradeLevelOptions = workspace.gradeLevels;
			programOptions = workspace.programs;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดห้องประจำชั้นไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function addHomeroom(draft: Omit<CreateHomeroomRequest, 'academicYearId'>) {
		if (!academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
		const created = await createHomeroom({
			academicYearId,
			...draft
		});
		homerooms = [...homerooms, created];
		advisorsByHomeroom.set(created.id, []);
		return created;
	}

	async function editHomeroom(room: Homeroom, draft: UpdateHomeroomRequest) {
		const updated = await updateHomeroom(room.id, draft);
		homerooms = homerooms.map((item) => (item.id === updated.id ? updated : item));
		return updated;
	}

	async function saveAdvisors(
		room: Homeroom,
		advisors: ReplaceHomeroomAdvisorsRequest['advisors']
	) {
		const savedAdvisors = await replaceHomeroomAdvisors(room.id, {
			rowVersion: room.rowVersion,
			advisors
		});
		advisorsByHomeroom.set(room.id, savedAdvisors);
		const refreshed = await getHomeroom(room.id);
		homerooms = homerooms.map((item) => (item.id === room.id ? refreshed : item));
		return savedAdvisors;
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
	title="ห้องประจำชั้น"
	description="ห้อง ที่ปรึกษา ความจุ และแผนการเรียนผูกกับปีที่เลือกโดยตรง"
>
	{#if !academicYearId}<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกบริบทบนแถบด้านบนเพื่อเปิดห้องของปีที่ต้องการ"
		/>{:else if loading}<PageSkeleton variant="cards" rows={5} />{:else if errorMessage}<PageState
			variant="error"
			title="โหลดห้องประจำชั้นไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicYearId)}
		/>{:else}<HomeroomEditor
			{homerooms}
			{gradeLevelOptions}
			{programOptions}
			{advisorsByHomeroom}
			{canManage}
			onCreate={addHomeroom}
			onUpdate={editHomeroom}
			onLoadStaffOptions={listStaffOptions}
			onSaveAdvisors={saveAdvisors}
		/>{/if}
</PageShell>
