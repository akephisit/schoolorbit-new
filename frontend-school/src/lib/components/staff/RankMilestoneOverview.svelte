<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { CalendarClock, RefreshCw, ArrowUpRight } from '@lucide/svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
	import {
		getRankMilestoneOverview,
		type RankMilestoneOverview,
		type RankMilestoneStatus,
		type PersonnelStatusFilter
	} from '$lib/api/personnel';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { formatCareerDate } from '$lib/forms/staff-career';
	import { ACADEMIC_RANK_LABELS } from '$lib/forms/staff-personnel';
	import { RANK_MILESTONE_LABELS } from '$lib/forms/rank-milestones';
	let {
		initial,
		status,
		ownerKey
	}: {
		initial: Promise<RouteLoadResult<RankMilestoneOverview | null>>;
		status: PersonnelStatusFilter;
		ownerKey: string;
	} = $props();
	let data = $state<RankMilestoneOverview | null>(null),
		loading = $state(true),
		error = $state(''),
		denied = $state(false);
	let bucket = $state<RankMilestoneStatus>('due_soon'),
		page = $state(1);
	let activeOwner = '',
		sourceOwner = '';
	let previousSource: typeof initial | undefined;
	const request = new LatestRequest();
	const cards = [
		{ value: 'due_soon', label: 'ใกล้ครบใน 90 วัน', count: 'dueSoon' },
		{
			value: 'time_reached_pending_review',
			label: 'ครบเวลา · รอตรวจคุณสมบัติ',
			count: 'timeReachedPendingReview'
		},
		{ value: 'incomplete', label: 'ข้อมูลยังไม่ครบ', count: 'incomplete' },
		{ value: 'future', label: 'ยังไม่ถึงกำหนด', count: 'future' },
		{ value: 'unsupported', label: 'เกณฑ์ที่ยังไม่รองรับ', count: 'unsupported' },
		{ value: 'no_next_rank', label: 'วิทยฐานะสูงสุด', count: 'noNextRank' }
	] as const;
	function apply(result: RouteLoadResult<RankMilestoneOverview | null>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			error = result.error;
			return;
		}
		denied = result.data === null;
		if (result.data) {
			data = result.data;
			bucket = data.bucket;
			page = data.page;
		}
		error = '';
	}
	$effect.pre(() => {
		const owner = `${ownerKey}:${status}`,
			source = initial;
		untrack(() => {
			if (owner !== activeOwner) {
				activeOwner = owner;
				data = null;
				bucket = 'due_soon';
				page = 1;
				denied = false;
			}
			if (!ownerKey || (source === previousSource && sourceOwner !== ownerKey)) {
				request.abort();
				loading = Boolean(ownerKey);
				return;
			}
			previousSource = source;
			sourceOwner = ownerKey;
			const ticket = request.begin();
			loading = true;
			error = '';
			void source.then((result) => apply(result, ticket.revision));
		});
		return () => request.abort();
	});
	onDestroy(() => request.abort());
	async function load(nextBucket = bucket, nextPage = page) {
		if (!ownerKey || denied) return;
		bucket = nextBucket;
		page = nextPage;
		const ticket = request.begin();
		loading = true;
		error = '';
		apply(
			await captureRouteLoad(
				getRankMilestoneOverview(
					{ status, bucket: nextBucket, page: nextPage },
					{ signal: ticket.signal }
				),
				'โหลดกำหนดเวลาวิทยฐานะไม่สำเร็จ'
			),
			ticket.revision
		);
	}
</script>

<section
	aria-label="ภาพรวมกำหนดเวลาวิทยฐานะ"
	aria-busy={loading}
	class="space-y-4 rounded-xl border bg-card p-5"
	data-testid="rank-milestone-overview"
>
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div class="flex items-center gap-3">
			<span
				class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
				><CalendarClock class="size-5" aria-hidden="true" /></span
			>
			<div>
				<h2 class="font-semibold">วางแผนวิทยฐานะ</h2>
				<p class="text-sm leading-relaxed text-muted-foreground">
					กำหนดเวลาตามเกณฑ์ปกติ สำหรับข้าราชการตำแหน่งครูในขอบเขตที่คุณดูได้
				</p>
			</div>
		</div>
		<Button
			variant="outline"
			size="sm"
			disabled={loading || denied}
			onclick={() => load()}
			aria-label="รีเฟรชกำหนดเวลาวิทยฐานะ"
			><RefreshCw
				class={loading ? 'size-4 motion-safe:animate-spin' : 'size-4'}
				aria-hidden="true"
			/>รีเฟรช</Button
		>
	</div>
	<p class="text-xs leading-relaxed text-muted-foreground">
		ครบระยะเวลาแล้ว ยังต้องตรวจคุณสมบัติและหลักฐานก่อนยื่น กรณีลดระยะเวลา พื้นที่พิเศษ
		หรือเส้นทางอื่น ให้ตรวจแยกในประวัติของแต่ละคน
	</p>
	{#if error}<PageState
			variant="error"
			title="โหลดกำหนดเวลาวิทยฐานะไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => load()}
		/>{/if}
	{#if denied}<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูข้อมูลกำหนดเวลาวิทยฐานะ"
		/>{:else if loading && !data}<PageSkeleton variant="cards" rows={3} />{:else if data}
		<div class="grid gap-3 sm:grid-cols-3" aria-label="เลือกกลุ่มกำหนดเวลา">
			{#each cards as card (card.value)}<Button
					variant={bucket === card.value ? 'secondary' : 'outline'}
					class="h-auto min-w-0 flex-col items-start gap-2 whitespace-normal p-4 text-left"
					aria-pressed={bucket === card.value}
					onclick={() => load(card.value, 1)}
					><span class="text-xs leading-relaxed">{card.label}</span><span
						class="text-2xl font-semibold tabular-nums"
						>{data.counts[card.count].toLocaleString('th-TH')}
						<span class="text-xs font-normal">คน</span></span
					></Button
				>{/each}
		</div>
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h3 class="text-sm font-semibold">{RANK_MILESTONE_LABELS[bucket]}</h3>
			<span class="text-xs text-muted-foreground"
				>{#if loading}<span role="status">กำลังอัปเดต…</span>{:else}{data.total.toLocaleString(
						'th-TH'
					)} คน{/if}</span
			>
		</div>
		{#if data.bucket === bucket && data.page === page}
			<ol class="divide-y">
				{#each data.items as person (person.staffId)}<li
						class="grid min-w-0 gap-3 py-4 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-center"
					>
						<div class="min-w-0 space-y-1">
							<p class="break-words text-sm font-medium">{person.displayName}</p>
							{#if person.milestone.nextRank}<p class="text-xs text-muted-foreground">
									เป้าหมาย: {ACADEMIC_RANK_LABELS[person.milestone.nextRank]}
								</p>{/if}
						</div>
						<div class="min-w-0 space-y-1">
							{#if person.milestone.ordinaryDate}<p class="text-sm font-medium leading-relaxed">
									{formatCareerDate(person.milestone.ordinaryDate)}
								</p>
								<p class="text-xs text-muted-foreground">ครบระยะเวลาตามเกณฑ์ปกติ</p>{:else}<ul
									class="space-y-1 text-xs leading-relaxed text-muted-foreground"
								>
									{#each person.milestone.reasonLabels as reason (reason)}<li>{reason}</li>{/each}
								</ul>{/if}
						</div>
						<Button
							variant="outline"
							size="sm"
							class="justify-self-start sm:justify-self-end"
							href={`/staff/manage/${person.staffId}?returnTo=${encodeURIComponent(`/staff/manage/overview?status=${status}`)}`}
							aria-label={`ดูประวัติ ${person.displayName}`}
							data-sveltekit-preload-data="tap"
							>ดูประวัติ<ArrowUpRight class="size-4" aria-hidden="true" /></Button
						>
					</li>{:else}<li class="py-6">
						<PageState
							variant="empty"
							title="ไม่มีบุคลากรในกลุ่มนี้"
							description="เลือกกลุ่มกำหนดเวลาอื่นเพื่อดูข้อมูล"
						/>
					</li>{/each}
			</ol>
		{/if}
		{#if data.total > data.pageSize}<div
				class="flex flex-wrap items-center justify-between gap-3 border-t pt-4"
			>
				<p class="text-xs text-muted-foreground">
					หน้า {page} จาก {Math.ceil(data.total / data.pageSize)} · หน้าละ {data.pageSize} คน
				</p>
				<div class="flex gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={loading || page <= 1}
						onclick={() => load(bucket, page - 1)}>ก่อนหน้า</Button
					><Button
						variant="outline"
						size="sm"
						disabled={loading || page * data.pageSize >= data.total}
						onclick={() => load(bucket, page + 1)}>ถัดไป</Button
					>
				</div>
			</div>{/if}
		<div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
			<Badge variant="outline">ใช้วันที่มีผลจริง</Badge><span
				>ข้อมูล ณ {formatCareerDate(data.asOf)} · รวม {data.filteredTotal} คนตามสถานะที่เลือก · เรียงตามวันครบระยะเวลา</span
			>
		</div>
	{/if}
</section>
