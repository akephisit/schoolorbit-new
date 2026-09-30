<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { getMyWorkItems } from '$lib/api/work';
	import { authStore } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { Badge, type BadgeVariant } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { workStore } from '$lib/stores/work';
	import { can } from '$lib/stores/permissions';
	import type { WorkItem, WorkItemState } from '$lib/api/work';
	import {
		AlertTriangle,
		CheckCircle2,
		Clock3,
		ExternalLink,
		LockKeyhole,
		TimerReset
	} from '@lucide/svelte';

	type WorkFilter = 'all' | WorkItemState;

	const filters: Array<{ value: WorkFilter; label: string }> = [
		{ value: 'all', label: 'ทั้งหมด' },
		{ value: 'open', label: 'เปิดอยู่' },
		{ value: 'due_soon', label: 'ใกล้ครบกำหนด' },
		{ value: 'overdue', label: 'เลยกำหนด' },
		{ value: 'submitted', label: 'ส่งแล้ว' },
		{ value: 'closed', label: 'ปิดแล้ว' }
	];

	let activeFilter = $state<WorkFilter>('all');

	let visibleItems = $derived.by(() => {
		if (activeFilter === 'all') return items;
		return items.filter((item) => item.state === activeFilter);
	});

	function filterCount(filter: WorkFilter): number | string {
		if (!itemsLoaded) return '—';
		if (filter === 'all') return items.length;
		return items.filter((item) => item.state === filter).length;
	}

	function stateLabel(state: WorkItemState): string {
		switch (state) {
			case 'scheduled':
				return 'รอเปิด';
			case 'open':
				return 'เปิดอยู่';
			case 'due_soon':
				return 'ใกล้ครบกำหนด';
			case 'overdue':
				return 'เลยกำหนด';
			case 'submitted':
				return 'ส่งแล้ว';
			case 'closed':
				return 'ปิดแล้ว';
			case 'archived':
				return 'เก็บถาวร';
		}
	}

	function stateVariant(state: WorkItemState): BadgeVariant {
		switch (state) {
			case 'due_soon':
			case 'overdue':
				return 'destructive';
			case 'submitted':
				return 'default';
			case 'closed':
			case 'archived':
				return 'outline';
			default:
				return 'secondary';
		}
	}

	function stateIcon(state: WorkItemState) {
		switch (state) {
			case 'due_soon':
			case 'overdue':
				return AlertTriangle;
			case 'submitted':
				return CheckCircle2;
			case 'closed':
			case 'archived':
				return LockKeyhole;
			case 'scheduled':
				return TimerReset;
			default:
				return Clock3;
		}
	}

	function formatDate(value?: string | null): string {
		if (!value) return '-';
		return new Date(value).toLocaleString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function itemTiming(item: WorkItem): string {
		if (item.submittedAt) return `ส่งเมื่อ ${formatDate(item.submittedAt)}`;
		if (item.dueAt) return `กำหนดส่ง ${formatDate(item.dueAt)}`;
		if (item.closesAt) return `ปิดรับ ${formatDate(item.closesAt)}`;
		if (item.opensAt) return `เปิด ${formatDate(item.opensAt)}`;
		return 'ไม่มีกำหนดเวลา';
	}

	let { data }: PageProps = $props();
	const source = $derived(data.items);
	let items = $state<WorkItem[]>([]),
		itemsLoading = $state(true),
		itemsLoaded = $state(false),
		itemsError = $state('');
	const itemsRequest = new LatestRequest();
	let observedRevision = 0,
		owner = '';
	function applyItems(result: Awaited<typeof data.items>, revision: number) {
		if (!itemsRequest.isCurrent(revision)) return;
		itemsLoading = false;
		if (result.ok) {
			items = result.data;
			itemsLoaded = true;
		} else itemsError = result.error;
	}
	async function loadItems() {
		if ($authStore.user?.user_type !== 'staff') return;
		const ticket = itemsRequest.begin();
		itemsLoading = true;
		itemsError = '';
		applyItems(
			await captureRouteLoad(
				getMyWorkItems({}, { signal: ticket.signal }),
				'โหลดรายการงานไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	$effect.pre(() => {
		const identity = $authStore.user?.id ?? '';
		untrack(() => {
			if (owner !== identity) {
				owner = identity;
				items = [];
				itemsLoaded = false;
				itemsRequest.abort();
			}
		});
	});
	$effect.pre(() => {
		const operation = source;
		untrack(() => {
			observedRevision = $workStore.revision;
			const ticket = itemsRequest.begin();
			itemsLoading = true;
			itemsError = '';
			void operation.then((result) => applyItems(result, ticket.revision));
		});
		return () => itemsRequest.abort();
	});
	$effect.pre(() => {
		const revision = $workStore.revision;
		untrack(() => {
			if (revision !== observedRevision) {
				observedRevision = revision;
				void loadItems();
			}
		});
	});
	onDestroy(() => itemsRequest.abort());
</script>

<PageShell
	title="งานของฉัน"
	description="งานที่ได้รับมอบหมายจากหน่วยงาน กลุ่มสาระ หรือรอบงานที่เปิดให้ดำเนินการ"
>
	{#snippet actions()}
		{#if $can.hasWorkflowManage()}
			<Button
				href="/staff/work/manage"
				data-sveltekit-preload-data="tap"
				variant="outline"
				size="sm">จัดการรอบงาน</Button
			>
		{/if}
	{/snippet}

	<section data-testid="work-counts" aria-busy={$workStore.loadingCounts}>
		{#if $workStore.loadingCounts}<p role="status" class="text-sm text-muted-foreground">
				กำลังโหลดจำนวนงาน...
			</p>{/if}
		{#if $workStore.countsError}<PageState
				variant="error"
				title="โหลดจำนวนงานไม่สำเร็จ"
				description={$workStore.countsError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => workStore.fetchCounts()}
			/>{/if}
		<div class="grid grid-cols-3 gap-2 sm:grid-cols-5">
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{#if !$workStore.loadedCounts && $workStore.loadingCounts}<Skeleton
							class="mx-auto h-6 w-8"
						/>{:else}{$workStore.loadedCounts ? $workStore.counts.open : '—'}{/if}
				</p>
				<p class="text-xs text-muted-foreground">เปิดอยู่</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold text-destructive">
					{#if !$workStore.loadedCounts && $workStore.loadingCounts}<Skeleton
							class="mx-auto h-6 w-8"
						/>{:else}{$workStore.loadedCounts ? $workStore.counts.dueSoon : '—'}{/if}
				</p>
				<p class="text-xs text-muted-foreground">ใกล้ครบ</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold text-destructive">
					{#if !$workStore.loadedCounts && $workStore.loadingCounts}<Skeleton
							class="mx-auto h-6 w-8"
						/>{:else}{$workStore.loadedCounts ? $workStore.counts.overdue : '—'}{/if}
				</p>
				<p class="text-xs text-muted-foreground">เลยกำหนด</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{#if !$workStore.loadedCounts && $workStore.loadingCounts}<Skeleton
							class="mx-auto h-6 w-8"
						/>{:else}{$workStore.loadedCounts ? $workStore.counts.submitted : '—'}{/if}
				</p>
				<p class="text-xs text-muted-foreground">ส่งแล้ว</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">
					{#if !$workStore.loadedCounts && $workStore.loadingCounts}<Skeleton
							class="mx-auto h-6 w-8"
						/>{:else}{$workStore.loadedCounts ? $workStore.counts.closed : '—'}{/if}
				</p>
				<p class="text-xs text-muted-foreground">ปิดแล้ว</p>
			</div>
		</div>
	</section>
	<Separator />

	<div class="flex flex-wrap gap-2 rounded-xl border bg-card p-3 sm:p-4">
		{#each filters as filter (filter.value)}
			<Button
				variant={activeFilter === filter.value ? 'default' : 'outline'}
				size="sm"
				onclick={() => {
					activeFilter = filter.value;
				}}
			>
				{filter.label}
				<span
					class="ml-1 rounded-full px-1.5 text-[11px] {activeFilter === filter.value
						? 'bg-primary-foreground/20'
						: 'bg-muted'}"
				>
					{filterCount(filter.value)}
				</span>
			</Button>
		{/each}
	</div>

	<section data-testid="work-items" aria-busy={itemsLoading}>
		{#if itemsLoading && itemsLoaded}<p role="status" class="text-sm text-muted-foreground">
				กำลังอัปเดตรายการงาน...
			</p>{/if}
		{#if itemsError}
			<PageState
				variant="error"
				title="โหลดงานไม่สำเร็จ"
				description={itemsError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadItems}
			/>
		{/if}
		{#if itemsLoading && !itemsLoaded}
			<div role="status" aria-label="กำลังโหลดรายการงาน">
				<PageSkeleton variant="cards" rows={4} />
			</div>
		{:else if itemsLoaded && visibleItems.length === 0}
			<PageState
				title="ยังไม่มีงานในสถานะนี้"
				description="เมื่องานจากฝ่ายงานหรือกลุ่มสาระเปิดให้ดำเนินการ งานจะแสดงที่นี่โดยไม่ทำให้เมนูหลักเปลี่ยนไปมา"
			/>
		{:else}
			<div class="grid gap-3">
				{#each visibleItems as item (item.id)}
					{@const StateIcon = stateIcon(item.state)}
					<article class="rounded-lg border bg-background p-4 transition-colors hover:bg-accent/40">
						<div class="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
							<div class="min-w-0 space-y-2">
								<div class="flex flex-wrap items-center gap-2">
									<Badge variant={stateVariant(item.state)}>
										<StateIcon class="h-3 w-3" />
										{stateLabel(item.state)}
									</Badge>
									<Badge variant="outline">{item.moduleCode}</Badge>
									{#if item.metadata.sourceLabel}
										<span class="text-xs text-muted-foreground">{item.metadata.sourceLabel}</span>
									{/if}
								</div>
								<div>
									<h2 class="text-base font-semibold text-foreground">{item.title}</h2>
									{#if item.description}
										<p class="mt-1 line-clamp-2 text-sm text-muted-foreground">
											{item.description}
										</p>
									{/if}
								</div>
								<p class="text-sm text-muted-foreground">{itemTiming(item)}</p>
							</div>

							<Button
								href={item.actionPath}
								data-sveltekit-preload-data="tap"
								variant="outline"
								size="sm"
							>
								<ExternalLink class="h-4 w-4" />
								{item.state === 'closed' || item.state === 'archived' ? 'เปิดดู' : 'ดำเนินการ'}
							</Button>
						</div>
					</article>
				{/each}
			</div>
		{/if}
	</section>
</PageShell>
