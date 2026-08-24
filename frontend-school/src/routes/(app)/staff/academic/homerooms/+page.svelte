<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createHomeroom,
		getHomeroom,
		listGradeLevelOptions,
		listHomeroomAdvisors,
		listHomerooms,
		listStaffOptions,
		listStudyProgramOptionsForYear,
		replaceHomeroomAdvisors,
		type Homeroom,
		type HomeroomAdvisor
	} from '$lib/api/academic-core';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import HomeroomEditor from '$lib/components/academic-core/HomeroomEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let homerooms = $state<Homeroom[]>([]);
	let gradeLevelOptions = $state<Array<{ id: string; name: string }>>([]);
	let programOptions = $state<Array<{ id: string; name: string }>>([]);
	const advisorsByHomeroom = new SvelteMap<string, HomeroomAdvisor[]>();
	let staffOptions = $state<Array<{ id: string; name: string }>>([]);
	let loading = $state(false);
	let errorMessage = $state('');
	let revision = 0;
	const canManage = $derived($can.has(PERMISSIONS.HOMEROOM_MANAGE_SCHOOL));

	async function loadWorkspace(yearId: string) {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const rooms = await listHomerooms(yearId);
			const nextAdvisors = new SvelteMap<string, HomeroomAdvisor[]>();
			for (const room of rooms) nextAdvisors.set(room.id, await listHomeroomAdvisors(room.id));
			const levels = await listGradeLevelOptions(yearId);
			const programs = await listStudyProgramOptionsForYear(yearId);
			const staff = await listStaffOptions();
			if (current !== revision) return;
			homerooms = rooms;
			advisorsByHomeroom.clear();
			for (const [roomId, advisors] of nextAdvisors) advisorsByHomeroom.set(roomId, advisors);
			gradeLevelOptions = levels.map((level) => ({ id: level.id, name: level.name }));
			programOptions = programs.map((program) => ({
				id: program.id,
				name: `${program.curriculumName} · ${program.name}`
			}));
			staffOptions = staff.map((person) => ({ id: person.id, name: person.name }));
		} catch (error) {
			if (current === revision)
				errorMessage = error instanceof Error ? error.message : 'โหลดห้องประจำชั้นไม่สำเร็จ';
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function addHomeroom(draft: {
		code: string;
		name: string;
		gradeLevelId: string;
		studyProgramId: string;
		roomNumber: string;
		capacity: number;
	}) {
		if (!academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
		const created = await createHomeroom({
			academicYearId,
			...draft,
			roomNumber: draft.roomNumber || null
		});
		homerooms = [...homerooms, created];
		advisorsByHomeroom.set(created.id, []);
	}

	async function addAdvisor(room: Homeroom, userId: string, role: string) {
		const existing = advisorsByHomeroom.get(room.id) ?? [];
		const advisors = await replaceHomeroomAdvisors(room.id, {
			rowVersion: room.rowVersion,
			advisors: [
				...existing.map((advisor) => ({ userId: advisor.userId, role: advisor.role })),
				{ userId, role }
			]
		});
		advisorsByHomeroom.set(room.id, advisors);
		const refreshed = await getHomeroom(room.id);
		homerooms = homerooms.map((item) => (item.id === room.id ? refreshed : item));
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
			{academicYearId}
			{homerooms}
			{gradeLevelOptions}
			{programOptions}
			{advisorsByHomeroom}
			{staffOptions}
			{canManage}
			onCreate={addHomeroom}
			onAddAdvisor={addAdvisor}
		/>{/if}
</PageShell>
