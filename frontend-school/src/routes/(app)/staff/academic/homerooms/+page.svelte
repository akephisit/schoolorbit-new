<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { HOMEROOMS_WORKSPACE_DEPENDENCY } from '$lib/academic-core/foundation-route';
	import {
		createHomeroom,
		getHomeroom,
		listStaffOptions,
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
	import HomeroomEditor from '$lib/components/academic-core/HomeroomEditor.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const academicYearId = $derived(data.academicYearId);
	let homerooms = $state<Homeroom[]>([]);
	let gradeLevelOptions = $state<GradeLevelOption[]>([]);
	let programOptions = $state<StudyProgramOption[]>([]);
	const advisorsByHomeroom = new SvelteMap<string, HomeroomAdvisor[]>();
	let loading = $state(true);
	let hasWorkspace = $state(false);
	let errorMessage = $state('');
	let loadedYearId: string | null = null;
	let mutationRevision = 0;
	const canManage = $derived($can.has(PERMISSIONS.HOMEROOM_MANAGE_SCHOOL));

	function retryWorkspace() {
		return invalidate(HOMEROOMS_WORKSPACE_DEPENDENCY);
	}

	$effect.pre(() => {
		const result = data.workspace;
		const yearId = data.academicYearId;
		const initialMutationRevision = mutationRevision;
		let current = true;
		untrack(() => {
			if (loadedYearId !== yearId) {
				loadedYearId = yearId;
				homerooms = [];
				advisorsByHomeroom.clear();
				gradeLevelOptions = [];
				programOptions = [];
				hasWorkspace = false;
			}
			loading = Boolean(result);
			errorMessage = '';
		});
		if (result) {
			void result.then((outcome) => {
				if (!current) return;
				untrack(() => {
					if (outcome.ok && mutationRevision === initialMutationRevision) {
						homerooms = outcome.data.homerooms;
						advisorsByHomeroom.clear();
						for (const [roomId, advisors] of outcome.data.advisorsByHomeroomId)
							advisorsByHomeroom.set(roomId, advisors);
						gradeLevelOptions = outcome.data.gradeLevels;
						programOptions = outcome.data.programs;
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

	async function addHomeroom(draft: Omit<CreateHomeroomRequest, 'academicYearId'>) {
		if (!academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
		const created = await createHomeroom({
			academicYearId,
			...draft
		});
		mutationRevision += 1;
		homerooms = [...homerooms, created];
		advisorsByHomeroom.set(created.id, []);
		return created;
	}

	async function editHomeroom(room: Homeroom, draft: UpdateHomeroomRequest) {
		const updated = await updateHomeroom(room.id, draft);
		mutationRevision += 1;
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
		mutationRevision += 1;
		advisorsByHomeroom.set(room.id, savedAdvisors);
		const refreshed = await getHomeroom(room.id);
		homerooms = homerooms.map((item) => (item.id === room.id ? refreshed : item));
		return savedAdvisors;
	}
</script>

<PageShell
	title="ห้องประจำชั้น"
	description="ห้อง ที่ปรึกษา ความจุ และแผนการเรียนผูกกับปีที่เลือกโดยตรง"
>
	{#if !academicYearId}<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="ใช้ตัวเลือกบริบทบนแถบด้านบนเพื่อเปิดห้องของปีที่ต้องการ"
		/>{:else if loading && !hasWorkspace}<PageSkeleton
			variant="cards"
			rows={5}
		/>{:else if errorMessage && !hasWorkspace}<PageState
			variant="error"
			title="โหลดห้องประจำชั้นไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={retryWorkspace}
		/>{:else}<div class="relative" aria-busy={loading} data-testid="homerooms-ready">
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตห้องประจำชั้น" />{/if}
			<HomeroomEditor
				{homerooms}
				{gradeLevelOptions}
				{programOptions}
				{advisorsByHomeroom}
				{canManage}
				onCreate={addHomeroom}
				onUpdate={editHomeroom}
				onLoadStaffOptions={listStaffOptions}
				onSaveAdvisors={saveAdvisors}
			/>
			{#if errorMessage && hasWorkspace}<div
					role="alert"
					class="mt-3 flex items-center gap-2 text-sm text-destructive"
				>
					<span>{errorMessage}</span><Button size="sm" variant="outline" onclick={retryWorkspace}
						>ลองอีกครั้ง</Button
					>
				</div>{/if}
		</div>{/if}
</PageShell>
