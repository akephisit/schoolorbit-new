<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		listCurriculumPublications,
		getCurriculumPublicationHistory,
		listCurriculumLevels,
		type CurriculumPublication,
		type CurriculumPublicationHistory,
		type CurriculumLevelView
	} from '#lib/api/academic-core.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let publications = $state.raw<CurriculumPublication[]>([]);
	let selectedId = $state('');
	let detail = $state.raw<CurriculumPublicationHistory | null>(null);
	let levels = $state.raw<CurriculumLevelView[]>([]);
	let loading = $state(true);
	let detailLoading = $state(false);
	let levelsLoading = $state(false);
	let error = $state('');
	let detailError = $state('');
	let levelsError = $state('');
	let visibleChanges = $state(100);
	const listRequest = new LatestRequest();
	const detailRequest = new LatestRequest();
	const levelsRequest = new LatestRequest();
	function date(value: string | null | undefined) {
		return value
			? new Date(value).toLocaleString('th-TH', {
					timeZone: 'Asia/Bangkok',
					dateStyle: 'medium',
					timeStyle: 'short'
				})
			: 'ไม่มีข้อมูลวันที่เผยแพร่เดิม';
	}
	$effect.pre(() => {
		const pending = data.publications;
		let current = true;
		untrack(() => {
			publications = [];
			selectedId = '';
			detail = null;
			levels = [];
			error = '';
			loading = true;
		});
		void pending.then((result) => {
			if (!current) return;
			if (result.ok) {
				publications = result.data;
				if (publications[0]) void select(publications[0].id);
			} else error = result.error;
			loading = false;
		});
		return () => {
			current = false;
			listRequest.abort();
			detailRequest.abort();
			levelsRequest.abort();
		};
	});
	async function retryList() {
		const editionId = data.editionId;
		const { revision, signal } = listRequest.begin();
		loading = true;
		error = '';
		try {
			const result = await listCurriculumPublications(editionId, { signal });
			if (listRequest.isCurrent(revision) && editionId === data.editionId) {
				publications = result;
				if (!selectedId && result[0]) void select(result[0].id);
			}
		} catch (e) {
			if (listRequest.isCurrent(revision) && !isAbortError(e))
				error = e instanceof Error ? e.message : 'โหลดประวัติไม่สำเร็จ';
		} finally {
			if (listRequest.isCurrent(revision)) loading = false;
		}
	}
	async function loadDetail(publicationId: string) {
		const editionId = data.editionId;
		const { revision, signal } = detailRequest.begin();
		detailLoading = true;
		detailError = '';
		try {
			const result = await getCurriculumPublicationHistory(editionId, publicationId, { signal });
			if (
				detailRequest.isCurrent(revision) &&
				editionId === data.editionId &&
				selectedId === publicationId
			)
				detail = result;
		} catch (e) {
			if (detailRequest.isCurrent(revision) && !isAbortError(e))
				detailError = e instanceof Error ? e.message : 'โหลดรายละเอียดไม่สำเร็จ';
		} finally {
			if (detailRequest.isCurrent(revision)) detailLoading = false;
		}
	}
	async function loadLevels(publicationId: string) {
		const editionId = data.editionId;
		const { revision, signal } = levelsRequest.begin();
		levelsLoading = true;
		levelsError = '';
		try {
			const result = await listCurriculumLevels(editionId, { publicationId, signal });
			if (
				levelsRequest.isCurrent(revision) &&
				editionId === data.editionId &&
				selectedId === publicationId
			)
				levels = result;
		} catch (e) {
			if (levelsRequest.isCurrent(revision) && !isAbortError(e))
				levelsError = e instanceof Error ? e.message : 'โหลดระดับไม่สำเร็จ';
		} finally {
			if (levelsRequest.isCurrent(revision)) levelsLoading = false;
		}
	}
	async function select(id: string) {
		selectedId = id;
		detail = null;
		levels = [];
		visibleChanges = 100;
		await Promise.all([loadDetail(id), loadLevels(id)]);
	}
</script>

<PageShell
	title="ประวัติการแก้ไขหลักสูตร"
	description="เลือกการเผยแพร่แต่ละครั้งเพื่อดูข้อมูลที่เก็บไว้และรายการเปลี่ยนแปลง"
>
	{#snippet actions()}<Button variant="outline" href={`/staff/academic/curricula/${data.editionId}`}
			>กลับฉบับหลักสูตร</Button
		>{/snippet}
	{#if loading && !publications.length}<PageSkeleton variant="cards" rows={3} />
	{:else if error && !publications.length}<PageState
			variant="error"
			title="โหลดประวัติไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retryList}
		/>
	{:else if !publications.length}<PageState
			variant="empty"
			title="ยังไม่เคยเผยแพร่หลักสูตร"
			description="ประวัติจะเริ่มเมื่อเผยแพร่ทั้งฉบับสำเร็จ"
		/>
	{:else}<div class="grid items-start gap-5 lg:grid-cols-[18rem_minmax(0,1fr)]">
			<section class="space-y-3" aria-label="รายการเผยแพร่">
				{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>
					<Button variant="outline" onclick={retryList}>ลองอีกครั้ง</Button>{/if}
				{#each publications as publication (publication.id)}
					<button
						type="button"
						class="w-full space-y-2 rounded-xl border p-4 text-left focus-visible:outline-2 focus-visible:outline-ring"
						class:bg-muted={selectedId === publication.id}
						aria-pressed={selectedId === publication.id}
						onclick={() => select(publication.id)}
					>
						<span class="block font-semibold">เผยแพร่ครั้งที่ {publication.publicationNo}</span>
						<span class="block text-sm text-muted-foreground">{date(publication.publishedAt)}</span>
						<span class="block break-words text-sm">{publication.changeNote}</span>
					</button>
				{/each}
			</section>
			<div class="min-w-0 space-y-5">
				{#if detailLoading && !detail}<PageSkeleton variant="cards" rows={2} />
				{:else if detailError && !detail}<PageState
						variant="error"
						title="โหลดรายละเอียดไม่สำเร็จ"
						description={detailError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => loadDetail(selectedId)}
					/>
				{:else if detail}<section
						class="space-y-4 rounded-2xl border bg-card p-4"
						data-testid="curriculum-publication-history"
					>
						<div class="flex flex-wrap items-center gap-2">
							<h2 class="font-semibold">
								{detail.publication.name} · เผยแพร่ครั้งที่ {detail.publication.publicationNo}
							</h2>
							<Badge variant="secondary">อ่านอย่างเดียว</Badge>
						</div>
						<p class="text-sm">{detail.publication.changeNote}</p>
						<p class="text-sm text-muted-foreground">
							ผู้เผยแพร่: {detail.publication.publisherName ?? 'ไม่มีข้อมูลผู้เผยแพร่เดิม'} · {date(
								detail.publication.publishedAt
							)}
						</p>
						<p class="text-sm">
							{detail.publication.levelCount} ระดับ · {detail.publication.programCount} แผน · {detail
								.publication.courseCount} รายการรายวิชา · {detail.publication.activityCount} รายการกิจกรรม
						</p>
						{#if detail.publication.isBaseline}<p class="rounded-xl bg-muted p-3 text-sm">
								ตั้งต้นจากข้อมูลจริงที่เผยแพร่อยู่ก่อนเริ่มเก็บประวัติการแก้ไข
							</p>
						{:else}<h3 class="font-medium">
								{detail.publication.previousPublicationId
									? 'เปลี่ยนจากการเผยแพร่ครั้งก่อน'
									: 'ข้อมูลที่เผยแพร่ครั้งแรก'}
							</h3>
							{#if !detail.changes.length}<p class="text-sm text-muted-foreground">
									โครงสร้างและรายวิชาเหมือนการเผยแพร่ครั้งก่อน
								</p>{/if}
							<ul class="space-y-3">
								{#each detail.changes.slice(0, visibleChanges) as change, i (`${selectedId}-${i}`)}<li
										class="space-y-1 rounded-xl border p-3 text-sm"
									>
										<p class="font-medium">
											{change.before == null ? 'เพิ่ม' : change.after == null ? 'นำออก' : 'แก้ไข'} · {change.name}
										</p>
										{#if change.before}<p class="break-words text-muted-foreground">
												ก่อน: {change.before}
											</p>{/if}
										{#if change.after}<p class="break-words">หลัง: {change.after}</p>{/if}
									</li>{/each}
							</ul>
							{#if detail.changes.length > visibleChanges}<Button
									variant="outline"
									onclick={() => (visibleChanges += 100)}>แสดงเพิ่มเติม</Button
								>{/if}
						{/if}
					</section>{/if}
				<section class="space-y-3 rounded-2xl border bg-card p-4">
					<h2 class="font-semibold">ดูแผนและเอกสารของการเผยแพร่ครั้งนี้</h2>
					{#if levelsLoading && !levels.length}<PageSkeleton variant="cards" rows={2} />
					{:else if levelsError}<PageState
							variant="error"
							title="โหลดระดับไม่สำเร็จ"
							description={levelsError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => loadLevels(selectedId)}
						/>
					{:else}{#each levels as { level } (level.id)}<Button
								variant="outline"
								href={`/staff/academic/curricula/${data.editionId}/levels/${level.id}?publicationId=${encodeURIComponent(selectedId)}`}
								>{level.nameTh}</Button
							>{/each}{/if}
				</section>
			</div>
		</div>{/if}
</PageShell>
