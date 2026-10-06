<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import {
		getCurriculumOverview,
		type CurriculumOverview,
		type CurriculumEdition
	} from '#lib/api/academic-core.js';
	import CurriculumCreateDialog from '#lib/components/academic-core/CurriculumCreateDialog.svelte';
	import CurriculumOverviewTable from '#lib/components/academic-core/CurriculumOverviewTable.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let overview = $state.raw<CurriculumOverview | null>(null);
	let loading = $state(true);
	let errorMessage = $state('');
	let mutationRevision = 0;
	const request = new LatestRequest();
	let canManageAcademicCurriculum = $derived(
		$can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL)
	);
	$effect.pre(() => {
		const pending = data.overview;
		let current = true;
		const revision = mutationRevision;
		untrack(() => {
			loading = true;
			errorMessage = '';
		});
		void pending.then((result) => {
			if (!current) return;
			if (result.ok) {
				if (revision === mutationRevision) overview = result.data;
			} else errorMessage = result.error;
			loading = false;
		});
		return () => {
			current = false;
			request.abort();
		};
	});
	function created(edition: CurriculumEdition) {
		mutationRevision++;
		overview = {
			items: [{ edition, levelCount: 0, studyProgramCount: 0 }, ...(overview?.items ?? [])]
		};
	}
	async function loadOverview() {
		const { revision, signal } = request.begin();
		const initialMutationRevision = mutationRevision;
		loading = true;
		errorMessage = '';
		try {
			const result = await getCurriculumOverview({ signal });
			if (request.isCurrent(revision) && initialMutationRevision === mutationRevision)
				overview = result;
		} catch (e) {
			if (request.isCurrent(revision) && !isAbortError(e))
				errorMessage = e instanceof Error ? e.message : 'โหลดรายการฉบับไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}
</script>

<PageShell
	title="หลักสูตรและแผนการเรียน"
	description="เลือกฉบับหลักสูตรก่อน แล้วจัดระดับการศึกษาและแผนการเรียนภายในฉบับนั้น"
>
	{#snippet actions()}{#if canManageAcademicCurriculum}<CurriculumCreateDialog
				onSaved={created}
			/>{/if}{/snippet}
	{#if loading && !overview}<PageSkeleton variant="table" rows={4} />
	{:else if errorMessage && !overview}<PageState
			variant="error"
			title="โหลดฉบับหลักสูตรไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadOverview}
		/>
	{:else if overview}<div
			class="relative space-y-4"
			aria-busy={loading}
			data-testid="curricula-overview-ready"
		>
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตรายการฉบับหลักสูตร" />{/if}
			{#if errorMessage && overview}<div
					role="alert"
					class="flex flex-wrap items-center gap-3 text-sm text-destructive"
				>
					<span>{errorMessage}</span><Button variant="outline" size="sm" onclick={loadOverview}
						>ลองอีกครั้ง</Button
					>
				</div>{/if}
			{#if overview.items.length}<CurriculumOverviewTable items={overview.items} />{:else}<PageState
					variant="empty"
					title="ยังไม่มีฉบับหลักสูตร"
					description="เพิ่มฉบับหลักสูตรและปีปรับปรุง แล้วกำหนดระดับการศึกษาและแผนการเรียน"
				/>{/if}
			<p class="text-sm text-muted-foreground">
				ปีปรับปรุงใช้ระบุฉบับหลักสูตร สามารถเลือกใช้ในปีการศึกษาถัดไปได้ตอนจัดห้อง
			</p>
		</div>{/if}
</PageShell>
