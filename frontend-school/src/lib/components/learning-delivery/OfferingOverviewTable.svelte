<script lang="ts">
	import type { LearningOfferingOverviewItem } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import {
		ArrowUpRight,
		BookOpen,
		Search,
		Sparkles,
		UserRoundCheck,
		UsersRound
	} from 'lucide-svelte';

	let {
		items,
		initialKind = 'all'
	}: {
		items: LearningOfferingOverviewItem[];
		initialKind?: 'all' | 'course' | 'activity';
	} = $props();

	let search = $state('');
	let kindFilter = $derived(initialKind);
	type AvailabilityState = 'draft' | 'upcoming' | 'active' | 'ended' | 'cancelled' | 'closed';
	let statusFilter = $state<'all' | AvailabilityState>('all');
	let gradeFilter = $state('all');
	let programFilter = $state('all');
	let gradeOptions = $derived.by(() => {
		const values: Array<[string, string]> = [];
		for (const item of items)
			for (const grade of item.gradeLevels)
				if (!values.some(([id]) => id === grade.id))
					values.push([grade.id, grade.short_name ?? grade.name]);
		return values.sort((left, right) =>
			left[1].localeCompare(right[1], 'th-TH', { numeric: true })
		);
	});
	let programOptions = $derived.by(() => {
		const values: Array<[string, string]> = [];
		for (const item of items)
			for (const program of item.studyPrograms)
				if (!values.some(([id]) => id === program.id))
					values.push([program.id, `${program.curriculumName} · ${program.name}`]);
		return values.sort((left, right) =>
			left[1].localeCompare(right[1], 'th-TH', { numeric: true })
		);
	});
	let filteredItems = $derived.by(() => {
		const query = search.trim().toLocaleLowerCase('th-TH');
		return items.filter((item) => {
			if (kindFilter !== 'all' && item.offering.kind !== kindFilter) return false;
			if (statusFilter !== 'all' && availabilityState(item) !== statusFilter) return false;
			if (gradeFilter !== 'all' && !item.gradeLevels.some((grade) => grade.id === gradeFilter))
				return false;
			if (
				programFilter !== 'all' &&
				!item.studyPrograms.some((program) => program.id === programFilter)
			)
				return false;
			if (!query) return true;
			return [
				item.offering.codeSnapshot,
				item.offering.nameSnapshot,
				...item.gradeLevels.flatMap((grade) => [grade.name, grade.short_name ?? '']),
				...item.studyPrograms.flatMap((program) => [program.name, program.curriculumName])
			].some((value) => value.toLocaleLowerCase('th-TH').includes(query));
		});
	});

	const statusLabel: Record<AvailabilityState, string> = {
		draft: 'ฉบับร่าง',
		upcoming: 'กำลังจะเริ่ม',
		active: 'กำลังสอน',
		ended: 'สิ้นสุดแล้ว',
		cancelled: 'ยกเลิกแล้ว',
		closed: 'ปิดแล้ว'
	};
	const statusClass: Record<AvailabilityState, string> = {
		draft: 'border-amber-500/35 bg-amber-500/10 text-amber-700 dark:text-amber-300',
		upcoming: 'border-blue-500/35 bg-blue-500/10 text-blue-700 dark:text-blue-300',
		active: 'border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
		ended: 'border-slate-500/30 bg-slate-500/10 text-slate-700 dark:text-slate-300',
		cancelled: 'border-rose-500/30 bg-rose-500/10 text-rose-700 dark:text-rose-300',
		closed: 'border-muted-foreground/25 bg-muted text-muted-foreground'
	};

	function todayIso(): string {
		const now = new Date();
		return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
	}

	function availabilityState(item: LearningOfferingOverviewItem): AvailabilityState {
		const offering = item.offering;
		if (offering.status === 'draft') return 'draft';
		if (offering.status === 'cancelled') return 'cancelled';
		if (offering.status === 'closed') return 'closed';
		const today = todayIso();
		if (offering.startsOn && offering.startsOn > today) return 'upcoming';
		if (offering.endsOn && offering.endsOn < today) return 'ended';
		return 'active';
	}

	function availabilityRange(item: LearningOfferingOverviewItem): string {
		const { startsOn, endsOn } = item.offering;
		if (!startsOn && !endsOn) return 'ใช้ตลอดภาคเรียน';
		if (startsOn && endsOn) return `${startsOn} – ${endsOn}`;
		if (startsOn) return `เริ่ม ${startsOn}`;
		return `ถึง ${endsOn}`;
	}

	function gradeSummary(item: LearningOfferingOverviewItem) {
		return item.gradeLevels.map((grade) => grade.short_name ?? grade.name).join(', ') || '—';
	}

	function programSummary(item: LearningOfferingOverviewItem) {
		if (item.studyPrograms.length === 0) return '—';
		if (item.studyPrograms.length <= 2)
			return item.studyPrograms.map((program) => program.name).join(', ');
		return `${item.studyPrograms[0].name} และอีก ${item.studyPrograms.length - 1} แผน`;
	}

	function courseWorkloadSummary(item: LearningOfferingOverviewItem) {
		if (item.offering.snapshot.kind !== 'course') return '';
		const snapshot = item.offering.snapshot;
		return `ตามหลักสูตร ${snapshot.standardPeriodsPerWeek} คาบ/สัปดาห์`;
	}
</script>

<div class="border-b bg-muted/15 p-4">
	<div class="grid gap-3 lg:grid-cols-[minmax(220px,1.5fr)_repeat(4,minmax(150px,0.75fr))]">
		<div class="relative">
			<Search
				class="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<Input
				bind:value={search}
				class="ps-9"
				placeholder="ค้นหารหัส ชื่อ ระดับชั้น หรือแผนการเรียน"
			/>
		</div>
		<Select.Root type="single" bind:value={kindFilter}>
			<Select.Trigger class="w-full" aria-label="กรองประเภทรายการเปิดสอน">
				{kindFilter === 'all' ? 'ทุกประเภท' : kindFilter === 'course' ? 'รายวิชา' : 'กิจกรรม'}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกประเภท</Select.Item>
				<Select.Item value="course">รายวิชา</Select.Item>
				<Select.Item value="activity">กิจกรรมพัฒนาผู้เรียน</Select.Item>
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={statusFilter}>
			<Select.Trigger class="w-full" aria-label="กรองสถานะรายการเปิดสอน">
				{statusFilter === 'all' ? 'ทุกช่วงการใช้งาน' : statusLabel[statusFilter]}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกช่วงการใช้งาน</Select.Item>
				<Select.Item value="draft">ฉบับร่าง</Select.Item>
				<Select.Item value="upcoming">กำลังจะเริ่ม</Select.Item>
				<Select.Item value="active">กำลังสอน</Select.Item>
				<Select.Item value="ended">สิ้นสุดแล้ว</Select.Item>
				<Select.Item value="cancelled">ยกเลิกแล้ว</Select.Item>
				<Select.Item value="closed">ปิดแล้ว</Select.Item>
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={gradeFilter}>
			<Select.Trigger class="w-full" aria-label="กรองระดับชั้น">
				{gradeFilter === 'all'
					? 'ทุกระดับชั้น'
					: (gradeOptions.find(([id]) => id === gradeFilter)?.[1] ?? 'ระดับชั้น')}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกระดับชั้น</Select.Item>
				{#each gradeOptions as [id, label] (id)}<Select.Item value={id}>{label}</Select.Item>{/each}
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={programFilter}>
			<Select.Trigger class="w-full" aria-label="กรองแผนการเรียน">
				{programFilter === 'all'
					? 'ทุกแผนการเรียน'
					: (programOptions.find(([id]) => id === programFilter)?.[1] ?? 'แผนการเรียน')}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกแผนการเรียน</Select.Item>
				{#each programOptions as [id, label] (id)}<Select.Item value={id}>{label}</Select.Item
					>{/each}
			</Select.Content>
		</Select.Root>
	</div>
</div>

{#if filteredItems.length === 0}
	<div class="p-10 text-center text-sm text-muted-foreground">
		ไม่พบรายการเปิดสอนที่ตรงกับตัวกรอง
	</div>
{:else}
	<div class="hidden overflow-x-auto lg:block">
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="min-w-[290px] ps-5">รายการเปิดสอน</Table.Head>
					<Table.Head class="min-w-[150px]">เป้าหมาย</Table.Head>
					<Table.Head class="min-w-[180px]">แผนการเรียน</Table.Head>
					<Table.Head class="text-center">กลุ่มเรียน</Table.Head>
					<Table.Head class="text-center">ครูหลัก</Table.Head>
					<Table.Head class="text-center">รายชื่อเผยแพร่</Table.Head>
					<Table.Head class="min-w-[145px]">ช่วงการใช้งาน</Table.Head>
					<Table.Head class="w-12"><span class="sr-only">เปิดรายละเอียด</span></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each filteredItems as item (item.offering.id)}
					<Table.Row>
						<Table.Cell class="border-s-4 border-s-primary ps-5">
							<div class="flex items-start gap-3">
								<div class="mt-0.5 rounded-lg bg-primary/10 p-2 text-primary">
									{#if item.offering.kind === 'course'}<BookOpen class="size-4" />{:else}<Sparkles
											class="size-4"
										/>{/if}
								</div>
								<div class="min-w-0">
									<p class="font-mono text-xs font-semibold text-primary">
										{item.offering.codeSnapshot}
									</p>
									<p class="font-medium">{item.offering.nameSnapshot}</p>
									<p class="text-xs text-muted-foreground">
										{item.offering.kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน'}
									</p>
									{#if item.offering.kind === 'course'}
										<p class="mt-1 text-xs font-medium text-primary">
											{courseWorkloadSummary(item)}
										</p>
									{/if}
								</div>
							</div>
						</Table.Cell>
						<Table.Cell>{gradeSummary(item)}</Table.Cell>
						<Table.Cell class="max-w-[220px] whitespace-normal">{programSummary(item)}</Table.Cell>
						<Table.Cell class="text-center font-mono tabular-nums">{item.groupCount}</Table.Cell>
						<Table.Cell class="text-center">
							<span
								class={item.groupsWithoutPrimaryTeacher > 0
									? 'font-medium text-amber-700'
									: 'text-emerald-700'}
							>
								{item.groupCount - item.groupsWithoutPrimaryTeacher}/{item.groupCount}
							</span>
							<p class="text-[11px] text-muted-foreground">มอบหมาย {item.teacherAssignmentCount}</p>
						</Table.Cell>
						<Table.Cell class="text-center font-mono tabular-nums"
							>{item.publishedRosterCount}/{item.groupCount}</Table.Cell
						>
						<Table.Cell>
							<Badge variant="outline" class={statusClass[availabilityState(item)]}>
								{statusLabel[availabilityState(item)]}
							</Badge>
							<p class="mt-1 text-[11px] text-muted-foreground">{availabilityRange(item)}</p>
						</Table.Cell>
						<Table.Cell>
							<Button
								href={`/staff/academic/delivery/${item.offering.id}`}
								size="icon"
								variant="ghost"
								aria-label={`เปิด ${item.offering.nameSnapshot}`}
							>
								<ArrowUpRight class="size-4" />
							</Button>
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>

	<div class="grid gap-3 p-4 lg:hidden">
		{#each filteredItems as item (item.offering.id)}
			<Button
				href={`/staff/academic/delivery/${item.offering.id}`}
				variant="outline"
				class="h-auto w-full justify-start rounded-xl border-s-4 border-s-primary bg-background p-4 text-start font-normal"
			>
				<div class="w-full space-y-3">
					<div class="flex items-start justify-between gap-3">
						<div>
							<p class="font-mono text-xs font-semibold text-primary">
								{item.offering.codeSnapshot}
							</p>
							<p class="font-medium">{item.offering.nameSnapshot}</p>
						</div>
						<ArrowUpRight class="size-4 shrink-0 text-muted-foreground" />
					</div>
					<div class="flex flex-wrap gap-1.5">
						<Badge variant="secondary"
							>{item.offering.kind === 'course' ? 'รายวิชา' : 'กิจกรรม'}</Badge
						><Badge variant="outline" class={statusClass[availabilityState(item)]}
							>{statusLabel[availabilityState(item)]}</Badge
						>
					</div>
					<p class="text-xs text-muted-foreground">{availabilityRange(item)}</p>
					{#if item.offering.kind === 'course'}
						<p class="text-xs font-medium text-primary">{courseWorkloadSummary(item)}</p>
					{/if}
					<p class="text-sm">{gradeSummary(item)} · {programSummary(item)}</p>
					<div class="grid grid-cols-3 gap-2 text-xs">
						<div class="rounded-lg bg-muted/50 p-2">
							<UsersRound class="mb-1 size-4 text-primary" />กลุ่ม {item.groupCount}
						</div>
						<div class="rounded-lg bg-muted/50 p-2">
							<UserRoundCheck class="mb-1 size-4 text-primary" />ขาดครู {item.groupsWithoutPrimaryTeacher}
						</div>
						<div class="rounded-lg bg-muted/50 p-2">
							รายชื่อ {item.publishedRosterCount}/{item.groupCount}
						</div>
					</div>
				</div>
			</Button>
		{/each}
	</div>
{/if}
