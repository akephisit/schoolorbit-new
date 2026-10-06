<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { groupCurriculaByRevision } from '#lib/academic-core/curriculum-presentation.js';
	import { untrack } from 'svelte';
	import { type CurriculumOverview, type CurriculumOverviewItem } from '#lib/api/academic-core.js';
	import { CURRICULUM_OVERVIEW_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
	import CurriculumCreateDialog from '#lib/components/academic-core/CurriculumCreateDialog.svelte';
	import CurriculumOverviewTable from '#lib/components/academic-core/CurriculumOverviewTable.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let overview = $state.raw<CurriculumOverview | null>(null);
	let loading = $state(true);
	let errorMessage = $state('');
	let mutationRevision = 0;
	let canManageAcademicCurriculum = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);
	let items = $derived(overview?.items ?? []);
	let revisionGroups = $derived(groupCurriculaByRevision(items));

	function loadOverview() {
		return invalidate(CURRICULUM_OVERVIEW_DEPENDENCY);
	}

	$effect.pre(() => {
		const routeResult = data.overview;
		const initialMutationRevision = mutationRevision;
		let current = true;
		untrack(() => {
			loading = Boolean(routeResult);
			errorMessage = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!current) return;
				untrack(() => {
					if (result.ok && mutationRevision === initialMutationRevision) overview = result.data;
					else if (!result.ok) errorMessage = result.error;
					loading = false;
				});
			});
		}
		return () => {
			current = false;
		};
	});

	function addCreatedCurriculum(item: CurriculumOverviewItem) {
		mutationRevision += 1;
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
</script>

<PageShell
	title="หลักสูตรและแผนการเรียน"
	description="เลือกฉบับปรับปรุงหลักสูตร ระดับการศึกษา และแผนการเรียน โรงเรียนกำหนดฉบับที่ใช้ตอนจัดห้อง"
>
	{#snippet actions()}
		{#if canManageAcademicCurriculum}
			<CurriculumCreateDialog onCreated={addCreatedCurriculum} />
		{/if}
	{/snippet}

	{#if loading && !overview}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !overview}
		<PageState
			variant="error"
			title="โหลดหลักสูตรไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOverview}
		/>
	{:else}
		<div
			class="relative space-y-4"
			aria-label="ภาพรวมหลักสูตร"
			aria-busy={loading}
			data-testid="curricula-overview-ready"
		>
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตภาพรวมหลักสูตร" />{/if}
			{#if items.length === 0}
				<PageState
					title="ยังไม่มีหลักสูตร"
					description="เพิ่มหลักสูตรแรกเพื่อเริ่มจัดฉบับหลักสูตร แผนการเรียน และรายวิชาในหลักสูตร"
				/>
			{:else}
				{#each revisionGroups as group (group.label)}
					<section class="overflow-hidden rounded-2xl border bg-card">
						<div class="flex items-start justify-between gap-4 border-b bg-muted/25 p-4">
							<div>
								<h2 class="font-semibold">หลักสูตรสถานศึกษา · {group.label}</h2>
								<p class="mt-1 text-sm text-muted-foreground">
									เลือกหลักสูตรเพื่อจัดฉบับหลักสูตร แผนการเรียน และข้อกำหนดรายวิชา
								</p>
							</div>
							<p
								class="shrink-0 rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary"
							>
								{group.items.length} หลักสูตร
							</p>
						</div>
						<CurriculumOverviewTable items={group.items} />
					</section>
				{/each}
				<p class="text-xs text-muted-foreground">
					ปีปรับปรุงระบุฉบับหลักสูตร ส่วนปีการศึกษาบนแถบด้านบนใช้กับห้องเรียนและการเปิดสอน
				</p>
			{/if}
			{#if errorMessage}<div
					role="alert"
					class="flex flex-wrap items-center gap-2 text-sm text-destructive"
				>
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={loadOverview}>ลองอีกครั้ง</Button>
				</div>{/if}
		</div>
	{/if}
</PageShell>
