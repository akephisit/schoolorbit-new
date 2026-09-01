<script lang="ts">
	import type {
		TimetableBlockPlacementCandidate,
		TimetableBlockPlacementSource,
		TimetableBlockWorkspaceLearningGroup,
		TimetableOrdinaryDemand,
		TimetableSynchronizedDemand
	} from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Popover from '$lib/components/ui/popover';
	import { Check, ChevronDown, GripVertical, Inbox, Plus, UsersRound } from 'lucide-svelte';

	type DemandSelection = {
		source: TimetableBlockPlacementSource;
		candidate: TimetableBlockPlacementCandidate;
	};

	let {
		ordinaryDemands,
		synchronizedDemands,
		groups,
		disabled = false,
		onChooseDemand,
		onDragStartDemand,
		onCancelDrag,
		onOpenStructural
	}: {
		ordinaryDemands: TimetableOrdinaryDemand[];
		synchronizedDemands: TimetableSynchronizedDemand[];
		groups: TimetableBlockWorkspaceLearningGroup[];
		disabled?: boolean;
		onChooseDemand: (
			source: TimetableBlockPlacementSource,
			candidate: TimetableBlockPlacementCandidate
		) => void;
		onDragStartDemand?: (
			source: TimetableBlockPlacementSource,
			candidate: TimetableBlockPlacementCandidate,
			event: DragEvent
		) => void;
		onCancelDrag?: () => void;
		onOpenStructural?: () => void;
	} = $props();

	let instructorChoices = $state<Record<string, string[]>>({});
	const groupById = $derived(new Map(groups.map((group) => [group.id, group])));
	const visibleOrdinary = $derived(ordinaryDemands.filter((demand) => demand.remainingPeriods > 0));
	const visibleSynchronized = $derived(
		synchronizedDemands.filter((demand) => demand.scheduledPeriods < demand.requiredPeriods)
	);

	function selectedInstructorIds(demand: TimetableOrdinaryDemand): string[] {
		const saved = instructorChoices[demand.learningGroupId];
		if (saved) return saved;
		if (demand.eligibleInstructors.length === 1) {
			return [demand.eligibleInstructors[0].teacherId];
		}
		return demand.eligibleInstructors
			.filter((teacher) => teacher.role === 'primary')
			.map((teacher) => teacher.teacherId);
	}

	function toggleInstructor(demand: TimetableOrdinaryDemand, teacherId: string): void {
		const selected = selectedInstructorIds(demand);
		instructorChoices = {
			...instructorChoices,
			[demand.learningGroupId]: selected.includes(teacherId)
				? selected.filter((id) => id !== teacherId)
				: [...selected, teacherId]
		};
	}

	function ordinarySelection(demand: TimetableOrdinaryDemand): DemandSelection {
		const group = groupById.get(demand.learningGroupId);
		return {
			source: {
				kind: 'ordinary_demand',
				learningGroupId: demand.learningGroupId,
				learningOfferingId: demand.learningOfferingId
			},
			candidate: {
				blockKind: group?.offeringKind === 'activity' ? 'activity' : 'course',
				learningGroupId: demand.learningGroupId,
				learningOfferingId: demand.learningOfferingId,
				roomId: null,
				instructorIds: selectedInstructorIds(demand),
				homeroomIds: demand.homeroomIds,
				teacherIds: []
			}
		};
	}

	function synchronizedSelection(demand: TimetableSynchronizedDemand): DemandSelection {
		return {
			source: {
				kind: 'synchronized_offering',
				learningOfferingId: demand.learningOfferingId
			},
			candidate: {
				blockKind: 'activity',
				learningGroupId: null,
				learningOfferingId: demand.learningOfferingId,
				roomId: null,
				instructorIds: [],
				homeroomIds: demand.intendedHomeroomIds,
				teacherIds: []
			}
		};
	}

	function dragOrdinary(demand: TimetableOrdinaryDemand, event: DragEvent): void {
		const selection = ordinarySelection(demand);
		if ((selection.candidate.instructorIds?.length ?? 0) === 0) {
			event.preventDefault();
			return;
		}
		event.dataTransfer?.setData('text/plain', demand.learningGroupId);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy';
		onDragStartDemand?.(selection.source, selection.candidate, event);
	}

	function dragSynchronized(demand: TimetableSynchronizedDemand, event: DragEvent): void {
		const selection = synchronizedSelection(demand);
		event.dataTransfer?.setData('text/plain', demand.learningOfferingId);
		if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy';
		onDragStartDemand?.(selection.source, selection.candidate, event);
	}
</script>

<aside class="overflow-hidden rounded-xl border bg-background" aria-label="คาบที่ยังไม่ได้จัด">
	<div class="flex items-center justify-between gap-3 border-b px-4 py-3">
		<div>
			<h2 class="font-semibold">ถาดคาบที่รอจัด</h2>
			<p class="text-xs text-muted-foreground">เลือกครูให้คาบนั้น แล้วลากลงสมุดตาราง</p>
		</div>
		<Badge variant="secondary">{visibleOrdinary.length + visibleSynchronized.length} รายการ</Badge>
	</div>
	<div class="border-b p-3">
		<Button
			type="button"
			variant="outline"
			class="w-full justify-start"
			{disabled}
			onclick={onOpenStructural}
		>
			<Plus class="size-4 text-amber-600" /> เพิ่มคาบพิเศษ
		</Button>
		<p class="mt-1.5 text-[0.7rem] text-muted-foreground">
			หน้าเสาธง โฮมรูม พัก ประชุมครู หรือกิจกรรมอื่นที่ไม่ใช่รายวิชา
		</p>
	</div>

	{#if visibleOrdinary.length === 0 && visibleSynchronized.length === 0}
		<div class="flex flex-col items-center gap-2 px-5 py-10 text-center text-muted-foreground">
			<Inbox class="size-7" />
			<p class="text-sm font-medium text-foreground">จัดครบตามเป้าหมายแล้ว</p>
			<p class="text-xs">หากเป้าหมายเปลี่ยน ให้ปรับจำนวนคาบจากหน้าจัดการเรียน</p>
		</div>
	{:else}
		<div class="max-h-[42rem] space-y-5 overflow-y-auto p-3">
			{#if visibleSynchronized.length > 0}
				<section class="space-y-2">
					<div class="flex items-center gap-2 px-1">
						<div class="h-4 w-1 rounded-full bg-violet-500"></div>
						<h3 class="text-xs font-semibold">กิจกรรมพร้อมกัน</h3>
					</div>
					{#each visibleSynchronized as demand (demand.learningOfferingId)}
						<article
							draggable={!disabled}
							class="cursor-grab rounded-lg border border-l-4 border-l-violet-500 bg-violet-50/40 p-3 active:cursor-grabbing dark:bg-violet-950/10"
							ondragstart={(event) => dragSynchronized(demand, event)}
							ondragend={onCancelDrag}
						>
							<div class="flex items-start gap-2">
								<GripVertical class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
								<button
									type="button"
									class="min-w-0 flex-1 text-left"
									{disabled}
									onclick={() => {
										const selection = synchronizedSelection(demand);
										onChooseDemand(selection.source, selection.candidate);
									}}
								>
									<p class="font-mono text-xs font-semibold text-violet-700 dark:text-violet-300">
										{demand.offeringCode}
									</p>
									<p class="text-sm font-medium">{demand.offeringName}</p>
									<p class="mt-1 text-[0.7rem] text-muted-foreground">
										พร้อมกัน {demand.intendedHomeroomIds.length} ห้อง · เหลือ
										{demand.requiredPeriods - demand.scheduledPeriods} คาบ
									</p>
								</button>
							</div>
						</article>
					{/each}
				</section>
			{/if}

			{#if visibleOrdinary.length > 0}
				<section class="space-y-2">
					<div class="flex items-center gap-2 px-1">
						<div class="h-4 w-1 rounded-full bg-primary"></div>
						<h3 class="text-xs font-semibold">รายวิชาและกิจกรรมรายกลุ่ม</h3>
					</div>
					{#each visibleOrdinary as demand (demand.learningGroupId)}
						{@const group = groupById.get(demand.learningGroupId)}
						{@const selectedIds = selectedInstructorIds(demand)}
						<article
							draggable={!disabled && selectedIds.length > 0}
							class="rounded-lg border border-l-4 border-l-primary bg-muted/15 p-3"
							ondragstart={(event) => dragOrdinary(demand, event)}
							ondragend={onCancelDrag}
						>
							<div class="flex items-start gap-2">
								<GripVertical
									class={`mt-0.5 size-4 shrink-0 ${selectedIds.length ? 'text-muted-foreground' : 'text-muted'}`}
								/>
								<button
									type="button"
									class="min-w-0 flex-1 text-left"
									disabled={disabled || selectedIds.length === 0}
									onclick={() => {
										const selection = ordinarySelection(demand);
										onChooseDemand(selection.source, selection.candidate);
									}}
								>
									<p class="font-mono text-xs font-semibold text-primary">{demand.offeringCode}</p>
									<p class="line-clamp-2 text-sm font-medium">{demand.offeringName}</p>
									<p class="mt-1 text-[0.7rem] text-muted-foreground">
										{group?.code ?? 'กลุ่มเรียน'} · เหลือ {demand.remainingPeriods}/{demand.requiredPeriods}
										คาบ
									</p>
								</button>
							</div>
							<Popover.Root>
								<Popover.Trigger>
									{#snippet child({ props })}
										<Button
											{...props}
											type="button"
											size="sm"
											variant="outline"
											class="mt-3 w-full justify-between"
											disabled={disabled || demand.eligibleInstructors.length === 0}
										>
											<span class="flex min-w-0 items-center gap-1.5">
												<UsersRound class="size-3.5" />
												<span class="truncate">
													{selectedIds.length > 0
														? `เลือกครู ${selectedIds.length} คน`
														: 'กรุณาเลือกครู'}
												</span>
											</span>
											<ChevronDown class="size-3.5" />
										</Button>
									{/snippet}
								</Popover.Trigger>
								<Popover.Content class="w-72 p-2" align="start">
									<p class="px-2 pb-2 text-xs font-medium">ครูที่สอนคาบนี้</p>
									{#if demand.eligibleInstructors.length === 0}
										<p class="px-2 py-3 text-xs text-destructive">
											ยังไม่ได้กำหนดครูในหน้าจัดการเรียน
										</p>
									{:else}
										{#each demand.eligibleInstructors as teacher (teacher.teacherId)}
											<Button
												type="button"
												variant="ghost"
												class="h-auto w-full justify-start px-2 py-2 text-left"
												aria-pressed={selectedIds.includes(teacher.teacherId)}
												onclick={() => toggleInstructor(demand, teacher.teacherId)}
											>
												<span
													class={[
														'flex size-4 shrink-0 items-center justify-center rounded border',
														selectedIds.includes(teacher.teacherId) &&
															'border-primary bg-primary text-primary-foreground'
													]}
												>
													{#if selectedIds.includes(teacher.teacherId)}<Check class="size-3" />{/if}
												</span>
												<span>
													<span class="block text-xs font-medium">{teacher.displayName}</span>
													<span class="block text-[0.68rem] text-muted-foreground">
														{teacher.role === 'primary'
															? 'ครูหลัก'
															: teacher.role === 'assistant'
																? 'ครูผู้ช่วย'
																: 'ครูร่วมสอน'}
													</span>
												</span>
											</Button>
										{/each}
									{/if}
								</Popover.Content>
							</Popover.Root>
						</article>
					{/each}
				</section>
			{/if}
		</div>
	{/if}
</aside>
