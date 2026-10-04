<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		listStaffCareerHistory,
		type StaffCareerHistoryPage,
		type StaffCareerMutationAck
	} from '#lib/api/staff-career.js';
	import {
		formatCareerDate,
		CAREER_KIND_LABELS,
		type StaffCareerEntry
	} from '#lib/forms/staff-career.js';
	import { staffCareerEntryLabel } from '#lib/forms/staff-personnel.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Plus, Pencil, RefreshCw, CalendarDays, History, LoaderCircle } from '@lucide/svelte';
	import StaffRankMilestoneCard from './StaffRankMilestoneCard.svelte';
	import StaffCareerEntryDialog from './StaffCareerEntryDialog.svelte';
	let {
		staffId,
		initial,
		canEdit,
		onCurrentChanged
	}: {
		staffId: string;
		initial: Promise<RouteLoadResult<StaffCareerHistoryPage | null>>;
		canEdit: boolean;
		onCurrentChanged: () => Promise<void>;
	} = $props();
	const request = new LatestRequest();
	let data = $state<StaffCareerHistoryPage | null>(null),
		loading = $state(true),
		error = $state(''),
		denied = $state(false);
	let activeOwner = '',
		disposed = false,
		dialogOpen = $state(false),
		selected = $state<StaffCareerEntry | null>(null);
	const currentFacts = $derived(
		data
			? [
					{ kind: 'personnel_type' as const, entry: data.current.personnelType },
					{ kind: 'job_position' as const, entry: data.current.jobPosition },
					{ kind: 'academic_rank' as const, entry: data.current.academicRank }
				]
			: []
	);
	function apply(
		result: RouteLoadResult<StaffCareerHistoryPage | null>,
		revision: number,
		append = false
	) {
		if (disposed || !request.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			error = result.error;
			return;
		}
		if (!result.data) {
			denied = true;
			return;
		}
		const latest = result.data,
			currentEntries = [
				latest.current.personnelType,
				latest.current.jobPosition,
				latest.current.academicRank
			].filter((entry) => entry !== null);
		const records = append && data ? [...data.items, ...latest.items] : latest.items;
		const unique = new Map(records.map((entry) => [entry.id, entry]));
		data = {
			...latest,
			items: [...unique.values()].map(
				(entry) =>
					currentEntries.find((current) => current.id === entry.id) ?? {
						...entry,
						isCurrent: false
					}
			)
		};
		error = '';
		denied = false;
	}
	$effect.pre(() => {
		const owner = staffId,
			source = initial;
		untrack(() => {
			if (owner !== activeOwner) {
				activeOwner = owner;
				data = null;
				dialogOpen = false;
				selected = null;
				denied = false;
			}
			const ticket = request.begin();
			loading = true;
			error = '';
			void source.then((result) => apply(result, ticket.revision));
		});
		return () => request.abort();
	});
	$effect.pre(() => {
		if (!canEdit) dialogOpen = false;
	});
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	async function load(append = false) {
		const owner = staffId,
			cursor = append ? data?.nextCursor : null;
		if (append && !cursor) return;
		const ticket = request.begin();
		loading = true;
		error = '';
		const result = await captureRouteLoad(
			listStaffCareerHistory(owner, { cursor: cursor ?? undefined }, { signal: ticket.signal }),
			'โหลดประวัติไม่สำเร็จ'
		);
		if (owner !== staffId) return;
		apply(result, ticket.revision, append);
	}
	async function saved(_ack: StaffCareerMutationAck) {
		const owner = staffId,
			currentChanged = selected?.isCurrent ?? false;
		dialogOpen = false;
		selected = null;
		try {
			await Promise.all([load(), currentChanged ? onCurrentChanged() : Promise.resolve()]);
		} catch (cause) {
			if (!disposed && owner === staffId)
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ';
		}
	}
</script>

<section
	aria-busy={loading}
	aria-label="ประวัติตำแหน่งและวิทยฐานะ"
	class="space-y-5 rounded-xl border bg-card p-4 sm:p-6"
>
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div>
			<h2 class="flex items-center gap-2 text-lg font-semibold">
				<History class="size-5 text-primary" />ประวัติตำแหน่งและวิทยฐานะ
			</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				ประเภทบุคลากร ตำแหน่ง และวิทยฐานะ พร้อมวันที่และคำสั่ง
			</p>
		</div>
		<div class="flex flex-wrap gap-2">
			{#if canEdit && !denied}<Button
					size="sm"
					disabled={loading}
					onclick={() => {
						selected = null;
						dialogOpen = true;
					}}><Plus class="size-4" />เพิ่มประวัติย้อนหลัง</Button
				>{/if}{#if data}<Button
					size="sm"
					variant="outline"
					disabled={loading}
					onclick={() => load()}
					aria-label="รีเฟรชประวัติ"
					><RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} />รีเฟรช</Button
				>{/if}
		</div>
	</div>
	{#if loading && !data}<div
			role="status"
			aria-label="กำลังโหลดประวัติ"
			data-testid="career-history-loading"
		>
			<PageSkeleton variant="cards" rows={3} /><PageSkeleton variant="detail" />
		</div>
	{:else if denied}<PageState variant="permission" title="ไม่มีสิทธิ์ดูประวัติบุคลากร" />
	{:else}
		{#if error}<PageState
				variant="error"
				title="โหลดประวัติไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => load()}
			/>{/if}
		{#if data}
			<StaffRankMilestoneCard milestone={data.rankMilestone} />
			<div class="grid min-w-0 gap-3 md:grid-cols-3" aria-label="ข้อมูลบุคลากรปัจจุบัน">
				{#each currentFacts as fact (fact.kind)}<div
						class="min-w-0 space-y-3 rounded-xl border bg-muted/25 p-4"
						data-testid={`career-current-${fact.kind}`}
					>
						<div class="flex items-center justify-between gap-2">
							<p class="text-sm text-muted-foreground">{CAREER_KIND_LABELS[fact.kind]}</p>
							<Badge variant="secondary">ปัจจุบัน</Badge>
						</div>
						<p class="break-words text-base font-semibold">
							{fact.entry ? staffCareerEntryLabel(fact.entry) : 'ยังไม่ระบุ'}
						</p>
						<p class="flex items-start gap-2 text-sm text-muted-foreground">
							<CalendarDays class="mt-0.5 size-4 shrink-0" /><span
								>มีผล {formatCareerDate(fact.entry?.effectiveDate ?? null)}</span
							>
						</p>
						{#if fact.entry?.orderNumber}<p class="break-words text-xs text-muted-foreground">
								คำสั่ง {fact.entry.orderNumber}
							</p>{/if}
					</div>{/each}
			</div>
			{#if loading}<p role="status" class="flex items-center gap-2 text-sm text-muted-foreground">
					<LoaderCircle class="size-4 animate-spin" />กำลังอัปเดตประวัติ...
				</p>{/if}
			{#if data.items.length === 0}<PageState
					title="ยังไม่มีประวัติ"
					description="ข้อมูลย้อนหลังและวันที่ตามคำสั่งจะแสดงที่นี่เมื่อมีการบันทึก"
				/>
			{:else}<ol class="space-y-3" aria-label="รายการประวัติบุคลากร">
					{#each data.items as entry (entry.id)}<li class="min-w-0 rounded-xl border p-4">
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0 space-y-2">
									<div class="flex flex-wrap items-center gap-2">
										<span class="text-xs font-medium text-muted-foreground"
											>{CAREER_KIND_LABELS[entry.fact.kind]}</span
										>{#if entry.isCurrent}<Badge>ปัจจุบัน</Badge
											>{/if}{#if entry.source === 'existing_record'}<Badge variant="outline"
												>ข้อมูลเดิม</Badge
											>{/if}{#if !entry.effectiveDate}<Badge variant="secondary"
												>วันที่ยังไม่ระบุ</Badge
											>{/if}
									</div>
									<h3 class="break-words font-semibold">{staffCareerEntryLabel(entry)}</h3>
								</div>
								{#if canEdit}<Button
										size="sm"
										variant="outline"
										disabled={loading}
										aria-label={`แก้ไขประวัติ ${CAREER_KIND_LABELS[entry.fact.kind]} ${staffCareerEntryLabel(entry)}`}
										onclick={() => {
											selected = entry;
											dialogOpen = true;
										}}><Pencil class="size-4" />แก้ไข</Button
									>{/if}
							</div>
							<dl class="mt-3 grid gap-3 text-sm sm:grid-cols-3">
								<div>
									<dt class="text-xs text-muted-foreground">วันที่มีผล</dt>
									<dd>{formatCareerDate(entry.effectiveDate)}</dd>
								</div>
								<div>
									<dt class="text-xs text-muted-foreground">วันที่ออกคำสั่ง</dt>
									<dd>{formatCareerDate(entry.orderDate)}</dd>
								</div>
								<div>
									<dt class="text-xs text-muted-foreground">เลขที่คำสั่ง</dt>
									<dd class="break-words">{entry.orderNumber ?? 'ยังไม่ระบุ'}</dd>
								</div>
							</dl>
							{#if entry.note}<p
									class="mt-3 break-words border-t pt-3 text-sm text-muted-foreground"
								>
									{entry.note}
								</p>{/if}
						</li>{/each}
				</ol>{/if}
			{#if data.nextCursor}<div class="flex justify-center">
					<Button variant="outline" disabled={loading} onclick={() => load(true)}
						>โหลดประวัติเพิ่ม</Button
					>
				</div>{/if}
		{/if}
	{/if}
	{#if canEdit}<StaffCareerEntryDialog
			{staffId}
			entry={selected}
			bind:open={dialogOpen}
			onSaved={saved}
		/>{/if}
</section>
