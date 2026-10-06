<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		getCurriculum,
		listCurriculumLevels,
		publishCurriculum,
		type CurriculumEdition,
		type CurriculumLevelView,
		type CurriculumLevel
	} from '#lib/api/academic-core.js';
	import CurriculumCreateDialog from '#lib/components/academic-core/CurriculumCreateDialog.svelte';
	import CurriculumLevelCreateDialog from '#lib/components/academic-core/CurriculumLevelCreateDialog.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let edition = $state.raw<CurriculumEdition | null>(null);
	let levels = $state.raw<CurriculumLevelView[]>([]);
	let editionLoading = $state(true);
	let levelsLoading = $state(true);
	let editionError = $state('');
	let levelsError = $state('');
	let publishing = $state(false);
	let actionError = $state('');
	let editionRevision = 0;
	let levelsRevision = 0;
	const editionRequest = new LatestRequest();
	const levelsRequest = new LatestRequest();
	let canManage = $derived($can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL));
	$effect.pre(() => {
		const ep = data.edition;
		const lp = data.levels;
		const edRevision = editionRevision;
		const lvRevision = levelsRevision;
		let current = true;
		untrack(() => {
			edition = null;
			levels = [];
			editionLoading = true;
			levelsLoading = true;
			editionError = '';
			levelsError = '';
			actionError = '';
			publishing = false;
		});
		void ep.then((r) => {
			if (!current) return;
			if (r.ok) {
				if (edRevision === editionRevision) edition = r.data;
			} else editionError = r.error;
			editionLoading = false;
		});
		void lp.then((r) => {
			if (!current) return;
			if (r.ok) {
				if (lvRevision === levelsRevision) levels = r.data;
			} else levelsError = r.error;
			levelsLoading = false;
		});
		return () => {
			current = false;
			editionRequest.abort();
			levelsRequest.abort();
		};
	});
	function saved(result: CurriculumEdition) {
		editionRevision++;
		edition = result;
	}
	function created(level: CurriculumLevel) {
		levelsRevision++;
		levels = [...levels, { level }];
	}
	async function retryEdition() {
		const id = data.editionId;
		const mutation = editionRevision;
		const { revision, signal } = editionRequest.begin();
		editionLoading = true;
		editionError = '';
		try {
			const result = await getCurriculum(id, { signal });
			if (
				editionRequest.isCurrent(revision) &&
				id === data.editionId &&
				mutation === editionRevision
			)
				edition = result;
		} catch (e) {
			if (editionRequest.isCurrent(revision) && !isAbortError(e))
				editionError = e instanceof Error ? e.message : 'โหลดฉบับไม่สำเร็จ';
		} finally {
			if (editionRequest.isCurrent(revision)) editionLoading = false;
		}
	}
	async function retryLevels() {
		const id = data.editionId;
		const mutation = levelsRevision;
		const { revision, signal } = levelsRequest.begin();
		levelsLoading = true;
		levelsError = '';
		try {
			const result = await listCurriculumLevels(id, { signal });
			if (levelsRequest.isCurrent(revision) && id === data.editionId && mutation === levelsRevision)
				levels = result;
		} catch (e) {
			if (levelsRequest.isCurrent(revision) && !isAbortError(e))
				levelsError = e instanceof Error ? e.message : 'โหลดระดับไม่สำเร็จ';
		} finally {
			if (levelsRequest.isCurrent(revision)) levelsLoading = false;
		}
	}
	async function publish() {
		if (!edition || publishing) return;
		const selected = edition;
		publishing = true;
		actionError = '';
		try {
			const result = await publishCurriculum(selected.id, { rowVersion: selected.rowVersion });
			if (data.editionId !== selected.id) return;
			saved(result);
			levelsRevision++;
			levels = levels.map(({ level }) => ({ level: { ...level, status: 'published' } }));
		} catch (e) {
			if (data.editionId === selected.id)
				actionError = e instanceof Error ? e.message : 'เผยแพร่ไม่สำเร็จ';
		} finally {
			if (data.editionId === selected.id) publishing = false;
		}
	}
</script>

<PageShell
	title={edition?.name ?? 'ฉบับหลักสูตร'}
	description="ฉบับหลักสูตร → ระดับการศึกษา → แผนการเรียน → ชั้น → ภาคเรียน"
>
	{#snippet actions()}<Button variant="outline" href="/staff/academic/curricula"
			>กลับรายการฉบับ</Button
		>{/snippet}
	<div class="space-y-5">
		{#if editionLoading && !edition}<PageSkeleton
				variant="cards"
				rows={1}
			/>{:else if editionError && !edition}<PageState
				variant="error"
				title="โหลดฉบับหลักสูตรไม่สำเร็จ"
				description={editionError}
				actionLabel="ลองอีกครั้ง"
				onaction={retryEdition}
			/>{/if}
		{#if edition}<section
				class="space-y-4 rounded-2xl border bg-card p-4"
				data-testid="curriculum-edition-ready"
			>
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div class="space-y-2">
						<h2 class="font-semibold">หลักสูตรสถานศึกษา · {edition.name}</h2>
						<Badge variant={edition.status === 'published' ? 'default' : 'secondary'}
							>{edition.status === 'published'
								? 'เผยแพร่แล้ว'
								: edition.status === 'draft'
									? 'ฉบับร่าง'
									: 'เก็บถาวร'}</Badge
						>
					</div>
					{#if canManage && edition.status === 'draft'}<div class="flex flex-wrap gap-2">
							<CurriculumCreateDialog {edition} onSaved={saved} /><LoadingButton
								loading={publishing}
								loadingLabel="กำลังเผยแพร่"
								onclick={publish}>เผยแพร่ฉบับหลักสูตร</LoadingButton
							>
						</div>{/if}
				</div>
				<p class="text-sm text-muted-foreground">
					ปีปรับปรุง {edition.revisionYear ?? 'ยังไม่กำหนด'} · โรงเรียนเลือกใช้ฉบับนี้ให้ห้องได้ในปีการศึกษาที่ต้องการ
				</p>
				{#if edition.description}<p class="text-sm">{edition.description}</p>{/if}
				{#if actionError}<p role="alert" class="text-sm text-destructive">{actionError}</p>{/if}
			</section>{/if}
		{#if editionError && edition}<div role="alert" class="flex gap-3 text-sm text-destructive">
				<span>{editionError}</span><Button variant="outline" onclick={retryEdition}
					>ลองอีกครั้ง</Button
				>
			</div>{/if}
		<section class="space-y-4">
			{#if levelsError && levels.length}<div
					role="alert"
					class="flex gap-3 text-sm text-destructive"
				>
					<span>{levelsError}</span><Button variant="outline" onclick={retryLevels}
						>ลองอีกครั้ง</Button
					>
				</div>{/if}
			<div class="flex flex-wrap items-center justify-between gap-3">
				<h2 class="text-lg font-semibold">ระดับการศึกษาในฉบับนี้</h2>
				{#if canManage && !levelsLoading && edition?.status === 'draft'}<CurriculumLevelCreateDialog
						editionId={edition.id}
						onCreated={created}
					/>{/if}
			</div>
			{#if levelsLoading && !levels.length}<PageSkeleton
					variant="table"
					rows={3}
				/>{:else if levelsError && !levels.length}<PageState
					variant="error"
					title="โหลดระดับการศึกษาไม่สำเร็จ"
					description={levelsError}
					actionLabel="ลองอีกครั้ง"
					onaction={retryLevels}
				/>
			{:else if levels.length}<div class="overflow-x-auto rounded-xl border">
					<Table.Root
						><Table.Header
							><Table.Row
								><Table.Head>ระดับการศึกษา</Table.Head><Table.Head>จำนวนชั้นที่ครอบคลุม</Table.Head
								></Table.Row
							></Table.Header
						><Table.Body
							>{#each levels as { level } (level.id)}<Table.Row
									><Table.Cell
										><a
											class="font-medium text-primary hover:underline"
											href={`/staff/academic/curricula/${data.editionId}/levels/${level.id}`}
											>{level.nameTh}</a
										></Table.Cell
									><Table.Cell>{level.gradeLevelIds.length} ชั้น</Table.Cell></Table.Row
								>{/each}</Table.Body
						></Table.Root
					>
				</div>
			{:else}<PageState
					variant="empty"
					title="ยังไม่มีระดับการศึกษาในฉบับนี้"
					description="เพิ่มระดับการศึกษา เช่น มัธยมศึกษาตอนต้น แล้วเลือกชั้นที่ครอบคลุม"
				/>{/if}
		</section>
	</div>
</PageShell>
