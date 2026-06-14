<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { Button } from '$lib/components/ui/button';
	import { Badge, type BadgeVariant } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import { workStore } from '$lib/stores/work';
	import { can } from '$lib/stores/permissions';
	import type { WorkItem, WorkItemState } from '$lib/api/work';
	import {
		AlertTriangle,
		CheckCircle2,
		Clock3,
		ExternalLink,
		Inbox,
		LoaderCircle,
		LockKeyhole,
		TimerReset
	} from 'lucide-svelte';

	const { data }: PageProps = $props();

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
		if (activeFilter === 'all') return $workStore.items;
		return $workStore.items.filter((item) => item.state === activeFilter);
	});

	function filterCount(filter: WorkFilter): number {
		if (filter === 'all') return $workStore.counts.total;
		return $workStore.items.filter((item) => item.state === filter).length;
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

	onMount(() => {
		void workStore.fetchCounts();
		void workStore.fetchItems();
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6 sm:px-6">
	<header class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
		<div class="space-y-1">
			<div class="flex items-center gap-2">
				<Inbox class="h-7 w-7 text-primary" />
				<h1 class="text-2xl font-bold text-foreground">งานของฉัน</h1>
			</div>
			<p class="text-sm text-muted-foreground">
				งานที่ได้รับมอบหมายจากหน่วยงาน กลุ่มสาระ หรือรอบงานที่เปิดให้ดำเนินการ
			</p>
			{#if $can.hasWorkflowManage()}
				<Button href="/staff/work/manage" variant="outline" size="sm" class="mt-2">
					จัดการรอบงาน
				</Button>
			{/if}
		</div>

		<div class="grid grid-cols-3 gap-2 sm:grid-cols-5">
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">{$workStore.counts.open}</p>
				<p class="text-xs text-muted-foreground">เปิดอยู่</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold text-destructive">{$workStore.counts.dueSoon}</p>
				<p class="text-xs text-muted-foreground">ใกล้ครบ</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold text-destructive">{$workStore.counts.overdue}</p>
				<p class="text-xs text-muted-foreground">เลยกำหนด</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">{$workStore.counts.submitted}</p>
				<p class="text-xs text-muted-foreground">ส่งแล้ว</p>
			</div>
			<div class="rounded-md border bg-background px-3 py-2 text-center">
				<p class="text-lg font-semibold">{$workStore.counts.closed}</p>
				<p class="text-xs text-muted-foreground">ปิดแล้ว</p>
			</div>
		</div>
	</header>

	<Separator />

	<div class="flex flex-wrap gap-2">
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

	{#if $workStore.loadingItems}
		<div class="flex min-h-64 items-center justify-center rounded-lg border border-dashed">
			<div class="flex items-center gap-2 text-muted-foreground">
				<LoaderCircle class="h-5 w-5 animate-spin" />
				<span>กำลังโหลดงาน</span>
			</div>
		</div>
	{:else if $workStore.error}
		<div
			class="rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive"
		>
			{$workStore.error}
		</div>
	{:else if visibleItems.length === 0}
		<div
			class="flex min-h-64 flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center"
		>
			<Inbox class="mb-3 h-10 w-10 text-muted-foreground" />
			<h2 class="text-base font-semibold">ยังไม่มีงานในสถานะนี้</h2>
			<p class="mt-1 max-w-md text-sm text-muted-foreground">
				เมื่องานจากฝ่ายงานหรือกลุ่มสาระเปิดให้ดำเนินการ
				งานจะแสดงที่นี่โดยไม่ทำให้เมนูหลักเปลี่ยนไปมา
			</p>
		</div>
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
									<p class="mt-1 line-clamp-2 text-sm text-muted-foreground">{item.description}</p>
								{/if}
							</div>
							<p class="text-sm text-muted-foreground">{itemTiming(item)}</p>
						</div>

						<Button href={item.actionPath} variant="outline" size="sm">
							<ExternalLink class="h-4 w-4" />
							{item.state === 'closed' || item.state === 'archived' ? 'เปิดดู' : 'ดำเนินการ'}
						</Button>
					</div>
				</article>
			{/each}
		</div>
	{/if}
</section>
