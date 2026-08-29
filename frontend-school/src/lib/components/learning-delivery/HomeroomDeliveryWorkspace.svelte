<script lang="ts">
	import type {
		HomeroomDeliveryItem,
		HomeroomDeliveryWorkspace as Workspace
	} from '$lib/api/learning-delivery';
	import {
		filterHomeroomDeliveryRooms,
		summarizeHomeroomDelivery,
		type HomeroomReadinessFilter
	} from '$lib/academic/homeroom-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import {
		ArrowUpRight,
		BookOpenCheck,
		ChevronDown,
		CircleAlert,
		Clock3,
		Search,
		UsersRound
	} from 'lucide-svelte';

	let { workspace }: { workspace: Workspace } = $props();
	let search = $state('');
	let readiness = $state<HomeroomReadinessFilter>('all');
	let summary = $derived(summarizeHomeroomDelivery(workspace.homerooms));
	let rooms = $derived(filterHomeroomDeliveryRooms(workspace.homerooms, search, readiness));

	const offeringLabels = {
		missing: 'ยังไม่เปิดสอน',
		draft: 'ฉบับร่าง',
		published: 'เผยแพร่แล้ว',
		closed: 'ปิดแล้ว'
	} as const;
	const groupLabels = {
		missing: 'ยังไม่จัดกลุ่ม',
		normal: 'แยกตามห้อง',
		combined: 'เรียนรวมหลายห้อง',
		split: 'แบ่งหลายกลุ่ม',
		deferred: 'รอรูปแบบการเลือก'
	} as const;
	const timetableLabels = {
		unscheduled: 'ยังไม่ลงตาราง',
		partly_scheduled: 'ลงตารางบางส่วน',
		scheduled: 'ลงตารางแล้ว'
	} as const;

	function itemNeedsAttention(item: HomeroomDeliveryItem) {
		return (
			item.offeringState === 'missing' ||
			item.groupMode === 'missing' ||
			(item.groupMode !== 'deferred' && item.teacherState === 'missing_primary') ||
			(item.groupMode !== 'deferred' && item.timetableState !== 'scheduled')
		);
	}

	function groupSummary(item: HomeroomDeliveryItem) {
		if (item.groups.length === 0) return groupLabels[item.groupMode];
		return item.groups.map((group) => group.name).join(', ');
	}

	function courseWorkloadSummary(item: HomeroomDeliveryItem) {
		if (item.resourceKind !== 'course' || item.standardPeriodsPerWeek == null) return '';
		if (item.weeklyPeriodTarget === item.standardPeriodsPerWeek) {
			return `ตามหลักสูตรและจัดจริง ${item.standardPeriodsPerWeek} คาบ/สัปดาห์`;
		}
		if (item.weeklyPeriodTarget == null) {
			return `ตามหลักสูตร ${item.standardPeriodsPerWeek} คาบ/สัปดาห์ · จัดจริงภาคเรียนนี้ยังไม่เปิดสอน`;
		}
		return `ตามหลักสูตร ${item.standardPeriodsPerWeek} · จัดจริงภาคเรียนนี้ ${item.weeklyPeriodTarget} คาบ/สัปดาห์`;
	}
</script>

<div class="space-y-4">
	<section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="สรุปความพร้อมรายห้อง">
		<div class="rounded-xl border bg-card p-4 shadow-sm">
			<p class="text-xs font-medium text-muted-foreground">ห้องประจำชั้น</p>
			<p class="mt-2 text-2xl font-semibold tabular-nums">{summary.roomCount}</p>
			<p class="text-xs text-muted-foreground">ห้องในปีการศึกษานี้</p>
		</div>
		<div class="rounded-xl border bg-card p-4 shadow-sm">
			<p class="text-xs font-medium text-muted-foreground">โครงสร้างที่ควรเปิด</p>
			<p class="mt-2 text-2xl font-semibold tabular-nums">{summary.expectedCount}</p>
			<p class="text-xs text-muted-foreground">รายวิชาและกิจกรรมรวมทุกห้อง</p>
		</div>
		<div class="rounded-xl border border-emerald-500/25 bg-emerald-500/[0.06] p-4 shadow-sm">
			<p class="text-xs font-medium text-emerald-700 dark:text-emerald-300">มีกลุ่มรองรับแล้ว</p>
			<p class="mt-2 text-2xl font-semibold tabular-nums text-emerald-700 dark:text-emerald-300">
				{summary.readyCount}/{summary.expectedCount}
			</p>
			<p class="text-xs text-muted-foreground">นับเมื่อมีรายการเปิดสอนและกลุ่มที่เชื่อมห้อง</p>
		</div>
		<div class="rounded-xl border border-amber-500/30 bg-amber-500/[0.07] p-4 shadow-sm">
			<p class="text-xs font-medium text-amber-700 dark:text-amber-300">ห้องที่ต้องตรวจ</p>
			<p class="mt-2 text-2xl font-semibold tabular-nums text-amber-700 dark:text-amber-300">
				{summary.attentionRoomCount}
			</p>
			<p class="text-xs text-muted-foreground">ยังไม่ครบตามโครงสร้างของภาคเรียน</p>
		</div>
	</section>

	<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
		<div class="border-b bg-muted/20 p-4">
			<div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
				<div>
					<h2 class="font-semibold">ตรวจการเปิดสอนทีละห้อง</h2>
					<p class="mt-1 text-sm text-muted-foreground">
						เริ่มจากห้องประจำชั้น แล้วตรวจรายการเปิดสอน กลุ่ม ครูหลัก และตารางสอนตามลำดับ
					</p>
				</div>
				<div class="grid gap-2 sm:grid-cols-[minmax(250px,1fr)_190px]">
					<div class="relative">
						<Search
							class="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
						/>
						<Input
							bind:value={search}
							class="ps-9"
							placeholder="ค้นหาห้อง ระดับชั้น หรือแผนการเรียน"
						/>
					</div>
					<Select.Root type="single" bind:value={readiness}>
						<Select.Trigger class="w-full" aria-label="กรองความพร้อมของห้อง">
							{readiness === 'all'
								? 'ทุกห้อง'
								: readiness === 'ready'
									? 'พร้อมตามโครงสร้าง'
									: 'ต้องตรวจเพิ่มเติม'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทุกห้อง</Select.Item>
							<Select.Item value="ready">พร้อมตามโครงสร้าง</Select.Item>
							<Select.Item value="attention">ต้องตรวจเพิ่มเติม</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</div>

		{#if rooms.length === 0}
			<div class="p-12 text-center text-sm text-muted-foreground">ไม่พบห้องที่ตรงกับตัวกรอง</div>
		{:else}
			<div class="divide-y">
				{#each rooms as room, index (room.homeroom.id)}
					<details class="group" open={index === 0}>
						<summary
							class="flex cursor-pointer list-none flex-col gap-3 p-4 transition-colors hover:bg-muted/25 sm:flex-row sm:items-center sm:justify-between [&::-webkit-details-marker]:hidden"
						>
							<div class="flex min-w-0 items-start gap-3">
								<div
									class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 font-semibold text-primary"
								>
									{index + 1}
								</div>
								<div class="min-w-0">
									<div class="flex flex-wrap items-center gap-2">
										<h3 class="font-semibold">{room.homeroom.name}</h3>
										<Badge variant="outline"
											>{room.gradeLevel.short_name ?? room.gradeLevel.name}</Badge
										>
									</div>
									<p class="mt-1 text-sm text-muted-foreground">
										{room.studyProgram.name} · {room.studyProgram.curriculumName}
									</p>
								</div>
							</div>
							<div class="flex items-center gap-3 sm:justify-end">
								<div class="text-end">
									<p class="font-mono text-sm font-semibold tabular-nums">
										{room.readyCount}/{room.expectedCount}
									</p>
									<p class="text-xs text-muted-foreground">มีกลุ่มรองรับ</p>
								</div>
								{#if room.expectedCount > 0 && room.readyCount === room.expectedCount}
									<Badge class="bg-emerald-600 text-white hover:bg-emerald-600"
										>ครบตามโครงสร้าง</Badge
									>
								{:else}
									<Badge
										variant="outline"
										class="border-amber-500/40 text-amber-700 dark:text-amber-300"
									>
										<CircleAlert class="size-3" /> ต้องตรวจ
									</Badge>
								{/if}
								<ChevronDown class="size-4 transition-transform group-open:rotate-180" />
							</div>
						</summary>

						<div class="border-t bg-background">
							{#each room.blockers as blocker (blocker.code)}
								<div class="m-4 rounded-xl border border-amber-500/30 bg-amber-500/[0.06] p-4">
									<p class="font-medium text-amber-800 dark:text-amber-200">{blocker.message}</p>
									<Button href={blocker.recoveryPath} variant="link" class="mt-1 h-auto p-0">
										ไปจัดโครงสร้างหลักสูตร <ArrowUpRight class="size-3.5" />
									</Button>
								</div>
							{/each}

							{#if room.items.length > 0}
								<div class="overflow-x-auto">
									<Table.Root>
										<Table.Header>
											<Table.Row>
												<Table.Head class="min-w-[260px] ps-5">ตามโครงสร้างหลักสูตร</Table.Head>
												<Table.Head class="min-w-[140px]">รายการเปิดสอน</Table.Head>
												<Table.Head class="min-w-[190px]">กลุ่มเรียน</Table.Head>
												<Table.Head class="min-w-[130px]">ครูหลัก</Table.Head>
												<Table.Head class="min-w-[150px]">ตารางสอน</Table.Head>
												<Table.Head class="w-12"><span class="sr-only">จัดการ</span></Table.Head>
											</Table.Row>
										</Table.Header>
										<Table.Body>
											{#each room.items as item (item.requirementId)}
												<Table.Row class={itemNeedsAttention(item) ? 'bg-amber-500/[0.035]' : ''}>
													<Table.Cell class="ps-5">
														<div class="flex items-start gap-3">
															<div class="mt-0.5 rounded-lg bg-primary/10 p-2 text-primary">
																<BookOpenCheck class="size-4" />
															</div>
															<div>
																<p class="font-mono text-xs font-semibold text-primary">
																	{item.code}
																</p>
																<p class="font-medium">{item.name}</p>
																<p class="text-xs text-muted-foreground">
																	{item.resourceKind === 'course'
																		? 'รายวิชา'
																		: 'กิจกรรมพัฒนาผู้เรียน'} ·
																	{item.requirementKind === 'required'
																		? 'บังคับ'
																		: 'เลือก/เพิ่มเติม'}
																</p>
																{#if item.resourceKind === 'course'}
																	<p class="mt-1 text-xs font-medium text-primary">
																		{courseWorkloadSummary(item)}
																	</p>
																{/if}
															</div>
														</div>
													</Table.Cell>
													<Table.Cell>
														<Badge
															variant="outline"
															class={item.offeringState === 'missing'
																? 'border-amber-500/40 text-amber-700'
																: ''}
														>
															{offeringLabels[item.offeringState]}
														</Badge>
													</Table.Cell>
													<Table.Cell>
														<p class="font-medium">{groupLabels[item.groupMode]}</p>
														<p class="max-w-[260px] truncate text-xs text-muted-foreground">
															{groupSummary(item)}
														</p>
													</Table.Cell>
													<Table.Cell>
														<div class="flex items-center gap-1.5 text-sm">
															<UsersRound class="size-3.5" />
															{item.groupMode === 'deferred' && item.groups.length === 0
																? 'รอจัดกลุ่ม'
																: item.teacherState === 'assigned'
																	? 'มอบหมายแล้ว'
																	: 'ยังไม่มีครูหลัก'}
														</div>
													</Table.Cell>
													<Table.Cell>
														<div class="flex items-center gap-1.5 text-sm">
															<Clock3 class="size-3.5" />
															{item.groupMode === 'deferred' && item.groups.length === 0
																? 'รอจัดกลุ่ม'
																: timetableLabels[item.timetableState]}
														</div>
													</Table.Cell>
													<Table.Cell>
														{#if item.offeringId}
															<Button
																href={`/staff/academic/delivery/${item.offeringId}`}
																size="icon"
																variant="ghost"
																aria-label={`จัดการ ${item.name}`}
															>
																<ArrowUpRight class="size-4" />
															</Button>
														{/if}
													</Table.Cell>
												</Table.Row>
											{/each}
										</Table.Body>
									</Table.Root>
								</div>
							{/if}
						</div>
					</details>
				{/each}
			</div>
		{/if}
	</section>

	{#if workspace.unlinked.length > 0}
		<section class="rounded-2xl border border-amber-500/30 bg-amber-500/[0.045] p-4">
			<div class="flex items-start gap-3">
				<CircleAlert class="mt-0.5 size-5 text-amber-700 dark:text-amber-300" />
				<div>
					<h2 class="font-semibold">
						รายการที่ยังเชื่อมกับห้องไม่ได้ {workspace.unlinked.length} รายการ
					</h2>
					<p class="mt-1 text-sm text-muted-foreground">
						ตรวจเป้าหมายของรายการเปิดสอน หรือกำหนดห้องให้กลุ่มเรียนก่อนนำไปจัดตาราง
					</p>
				</div>
			</div>
			<div class="mt-3 grid gap-2 md:grid-cols-2">
				{#each workspace.unlinked as item (`${item.offeringId}-${item.groupId ?? 'offering'}`)}
					<div class="rounded-lg border bg-background p-3">
						<p class="font-mono text-xs font-semibold text-primary">{item.code}</p>
						<p class="font-medium">{item.name}</p>
						<p class="mt-1 text-xs text-muted-foreground">{item.reason}</p>
					</div>
				{/each}
			</div>
		</section>
	{/if}
</div>
