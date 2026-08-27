<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getCurriculumOverview,
		type CurriculumOverview,
		type CurriculumOverviewItem
	} from '$lib/api/academic-core';
	import CurriculumCreateDialog from '$lib/components/academic-core/CurriculumCreateDialog.svelte';
	import CurriculumOverviewTable from '$lib/components/academic-core/CurriculumOverviewTable.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let overview = $state.raw<CurriculumOverview | null>(null);
	let loading = $state(true);
	let errorMessage = $state('');
	let canManageAcademicCurriculum = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);
	let items = $derived(overview?.items ?? []);

	async function loadOverview() {
		loading = true;
		errorMessage = '';
		try {
			overview = await getCurriculumOverview();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดภาพรวมหลักสูตรไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	function addCreatedCurriculum(item: CurriculumOverviewItem) {
		if (!overview) {
			overview = { items: [item] };
			return;
		}
		overview = {
			...overview,
			items: [...overview.items, item].sort((left, right) =>
				left.curriculum.code.localeCompare(right.curriculum.code, 'th-TH', { numeric: true })
			)
		};
	}

	onMount(() => {
		void loadOverview();
	});
</script>

<PageShell
	title="หลักสูตรและแผนการเรียน"
	description="เห็นหลักสูตร รุ่นที่ใช้อยู่ ระดับชั้น และจำนวนแผนการเรียนในภาพรวม ก่อนเปิดจัดการรายละเอียด"
>
	{#snippet actions()}
		{#if canManageAcademicCurriculum}
			<CurriculumCreateDialog onCreated={addCreatedCurriculum} />
		{/if}
	{/snippet}

	{#if loading}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && items.length === 0}
		<PageState
			variant="error"
			title="โหลดหลักสูตรไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOverview}
		/>
	{:else if items.length === 0}
		<PageState
			title="ยังไม่มีหลักสูตร"
			description="เพิ่มหลักสูตรแรกเพื่อเริ่มจัดรุ่น แผนการเรียน และรายวิชาในหลักสูตร"
		/>
	{:else}
		<div class="space-y-4">
			<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
				<div class="flex items-start justify-between gap-4 border-b bg-muted/25 p-4">
					<div>
						<h2 class="font-semibold">ภาพรวมหลักสูตร</h2>
						<p class="mt-1 text-sm text-muted-foreground">
							เลือกหลักสูตรเพื่อจัดรุ่น แผนการเรียน และข้อกำหนดรายวิชา
						</p>
					</div>
					<p class="shrink-0 rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary">
						{items.length} หลักสูตร
					</p>
				</div>
				<CurriculumOverviewTable {items} />
			</section>
			<p class="text-xs text-muted-foreground">
				หลักสูตรเป็นข้อมูลกลางของโรงเรียนและไม่เปลี่ยนตามภาคเรียนบนแถบด้านบน
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>
