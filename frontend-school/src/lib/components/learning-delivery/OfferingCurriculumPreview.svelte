<script lang="ts">
	import {
		applyLearningOfferingsFromCurriculum,
		previewLearningOfferingsFromCurriculum,
		type CurriculumGroupProposal,
		type CurriculumOfferingPreview,
		type CurriculumPreparationChoice,
		type CurriculumPreparationProposal,
		type DeliveryManagementOptions
	} from '$lib/api/learning-delivery';
	import {
		buildFocusedCurriculumPreparationChoices,
		visibleCurriculumPreparationProposals,
		type SynchronizedActivityPreparationTarget
	} from '$lib/academic/synchronized-activity-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Command from '$lib/components/ui/command';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import * as Select from '$lib/components/ui/select';
	import {
		ChevronsUpDown,
		CircleAlert,
		Combine,
		Eye,
		Plus,
		RotateCcw,
		Trash2,
		WandSparkles
	} from 'lucide-svelte';

	let {
		academicTermId,
		options,
		onApplied,
		preparationTarget = null
	}: {
		academicTermId: string;
		options: DeliveryManagementOptions;
		onApplied: () => Promise<void> | void;
		preparationTarget?: SynchronizedActivityPreparationTarget | null;
	} = $props();

	let programPickerOpen = $state(false);
	let programSearch = $state('');
	let selectedStudyProgramIds = $state<string[]>([]);
	let studyProgramIds = $derived(preparationTarget?.studyProgramIds ?? selectedStudyProgramIds);
	let preview = $state.raw<CurriculumOfferingPreview | null>(null);
	let choices = $state.raw<CurriculumPreparationChoice[]>([]);
	let busy = $state(false);
	let errorMessage = $state('');
	let filteredPrograms = $derived.by(() => {
		const query = programSearch.trim().toLocaleLowerCase('th-TH');
		if (!query) return options.studyPrograms;
		return options.studyPrograms.filter((program) =>
			`${program.curriculumName} ${program.code} ${program.name}`
				.toLocaleLowerCase('th-TH')
				.includes(query)
		);
	});
	let selectedProgramLabel = $derived.by(() => {
		if (studyProgramIds.length === 0) return 'เลือกแผนการเรียน';
		if (studyProgramIds.length === 1)
			return (
				options.studyPrograms.find((program) => program.id === studyProgramIds[0])?.name ??
				'เลือกแล้ว 1 แผน'
			);
		return `เลือกแล้ว ${studyProgramIds.length} แผน`;
	});
	let displayedProposals = $derived(
		visibleCurriculumPreparationProposals(preview?.proposals ?? [], preparationTarget)
	);
	let applyBlocked = $derived.by(() => {
		if (!preview || choices.length !== preview.proposals.length) return true;
		if (preparationTarget) {
			if (displayedProposals.length !== 1) return true;
			const focusedChoice = choices.find(
				(choice) => choice.proposalId === displayedProposals[0]?.proposalId
			);
			if (!focusedChoice || focusedChoice.action === 'skip') return true;
		}
		return preview.proposals.some((proposal) => {
			const choice = choices.find((item) => item.proposalId === proposal.proposalId);
			if (!choice) return true;
			if (choice.action === 'apply' && proposal.conflicts.length > 0) return true;
			if (choice.action === 'apply' && choice.groups.length === 0) return true;
			return choice.groups.some((group) => !group.name.trim() || group.homeroomIds.length === 0);
		});
	});

	function toggleProgram(id: string) {
		selectedStudyProgramIds = studyProgramIds.includes(id)
			? studyProgramIds.filter((programId) => programId !== id)
			: [...studyProgramIds, id];
		preview = null;
		choices = [];
	}

	function homeroomLabel(homeroomId: string) {
		return options.homerooms.find((item) => item.id === homeroomId)?.name ?? 'ไม่พบชื่อห้อง';
	}

	function randomGroupKey() {
		return crypto.randomUUID().replaceAll('-', '').repeat(2);
	}

	function choiceFor(proposalId: string) {
		return choices.find((choice) => choice.proposalId === proposalId);
	}

	function updateChoice(proposalId: string, update: (choice: CurriculumPreparationChoice) => void) {
		choices = choices.map((choice) => {
			if (choice.proposalId !== proposalId) return choice;
			const next = { ...choice, groups: choice.groups.map((group) => ({ ...group })) };
			update(next);
			return next;
		});
	}

	function setAction(
		proposal: CurriculumPreparationProposal,
		action: CurriculumPreparationChoice['action']
	) {
		updateChoice(proposal.proposalId, (choice) => {
			choice.action = action;
			if (action !== 'apply') choice.groups = [];
			else if (choice.groups.length === 0)
				choice.groups = proposal.defaultGroups.map((group) => ({ ...group }));
		});
	}

	function restoreDefaults(proposal: CurriculumPreparationProposal) {
		updateChoice(proposal.proposalId, (choice) => {
			choice.action = 'apply';
			choice.groups = proposal.defaultGroups.map((group) => ({ ...group }));
		});
	}

	function combineGroups(proposal: CurriculumPreparationProposal) {
		const homeroomIds = [...new Set(proposal.targetHomeroomIds)].sort();
		if (homeroomIds.length < 2) return;
		updateChoice(proposal.proposalId, (choice) => {
			choice.action = 'apply';
			choice.groups = [
				{
					groupKey: randomGroupKey(),
					name: `${proposal.code} · เรียนรวม ${homeroomIds.map(homeroomLabel).join(', ')}`,
					homeroomIds
				}
			];
		});
	}

	function addSplitGroup(proposal: CurriculumPreparationProposal) {
		const homeroomId = proposal.targetHomeroomIds[0];
		if (!homeroomId) return;
		updateChoice(proposal.proposalId, (choice) => {
			choice.action = 'apply';
			choice.groups = [
				...choice.groups,
				{
					groupKey: randomGroupKey(),
					name: `${proposal.code} · ${homeroomLabel(homeroomId)} กลุ่ม ${choice.groups.length + 1}`,
					homeroomIds: [homeroomId]
				}
			];
		});
	}

	function updateGroupName(proposalId: string, groupKey: string, name: string) {
		updateChoice(proposalId, (choice) => {
			choice.groups = choice.groups.map((group) =>
				group.groupKey === groupKey ? { ...group, name } : group
			);
		});
	}

	function removeGroup(proposalId: string, groupKey: string) {
		updateChoice(proposalId, (choice) => {
			choice.groups = choice.groups.filter((group) => group.groupKey !== groupKey);
		});
	}

	function toggleGroupHomeroom(proposalId: string, groupKey: string, homeroomId: string) {
		updateChoice(proposalId, (choice) => {
			choice.groups = choice.groups.map((group) => {
				if (group.groupKey !== groupKey) return group;
				return {
					...group,
					homeroomIds: group.homeroomIds.includes(homeroomId)
						? group.homeroomIds.filter((id) => id !== homeroomId)
						: [...group.homeroomIds, homeroomId].sort()
				};
			});
		});
	}

	function groupRooms(group: CurriculumGroupProposal) {
		return group.homeroomIds.map(homeroomLabel).join(', ');
	}

	async function buildPreview() {
		if (studyProgramIds.length === 0) return;
		busy = true;
		errorMessage = '';
		try {
			const result = await previewLearningOfferingsFromCurriculum({
				academicTermId,
				studyProgramIds
			});
			const focusedProposals = visibleCurriculumPreparationProposals(
				result.proposals,
				preparationTarget
			);
			if (preparationTarget && focusedProposals.length !== 1) {
				preview = null;
				choices = [];
				errorMessage = `ไม่พบ ${preparationTarget.code} · ${preparationTarget.name} ในโครงสร้างของห้องเป้าหมาย`;
				return;
			}
			preview = result;
			choices = buildFocusedCurriculumPreparationChoices(result.proposals, preparationTarget);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างตัวอย่างจากหลักสูตรไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function applyPreview() {
		if (!preview || applyBlocked) return;
		busy = true;
		errorMessage = '';
		try {
			await applyLearningOfferingsFromCurriculum({
				academicTermId,
				studyProgramIds,
				sourceHash: preview.sourceHash,
				idempotencyKey: crypto.randomUUID(),
				choices
			});
			await onApplied();
			preview = null;
			choices = [];
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'นำรายการจากหลักสูตรมาใช้ไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="space-y-4">
	<div class="rounded-xl border border-primary/20 bg-primary/[0.035] p-4">
		<div class="flex items-start gap-3">
			<div class="rounded-lg bg-primary/10 p-2 text-primary"><WandSparkles class="size-4" /></div>
			<div>
				<h3 class="font-medium">
					{preparationTarget
						? `เปิดใช้งาน ${preparationTarget.code} · ${preparationTarget.name}`
						: 'เตรียมรายการเปิดสอนและกลุ่มจากหลักสูตร'}
				</h3>
				<p class="mt-1 text-sm text-muted-foreground">
					{preparationTarget
						? `ระบบจะรวมข้อกำหนดของ ${preparationTarget.homeroomCount} ห้องเป็นรายการเปิดสอนกลางเพียงรายการเดียว`
						: 'ระบบเสนอหนึ่งกลุ่มต่อห้องสำหรับรายการบังคับ ส่วนรายการเลือกจะรอให้จัดกลุ่มภายหลัง'}
				</p>
			</div>
		</div>
	</div>

	<div class="grid gap-4 sm:grid-cols-2">
		<div class="space-y-2">
			<Label>{preparationTarget ? 'ขอบเขตจากหลักสูตร' : 'แผนการเรียน'}</Label>
			{#if preparationTarget}
				<div class="rounded-md border bg-muted/25 px-3 py-2 text-sm">
					<p class="font-medium">{selectedProgramLabel} · {preparationTarget.homeroomCount} ห้อง</p>
					<p class="mt-0.5 text-xs text-muted-foreground">
						ระบบเลือกทุกแผนการเรียนที่มีกิจกรรมนี้ไว้แล้ว
					</p>
				</div>
			{:else}
				<Popover.Root bind:open={programPickerOpen}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button
								type="button"
								variant="outline"
								role="combobox"
								aria-label="เลือกแผนการเรียนจากหลักสูตร"
								aria-expanded={programPickerOpen}
								class="w-full justify-between font-normal"
								{...props}
							>
								<span class="truncate">{selectedProgramLabel}</span><ChevronsUpDown
									class="size-4 opacity-50"
								/>
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
						<Command.Root shouldFilter={false}>
							<Command.Input
								bind:value={programSearch}
								placeholder="ค้นหาหลักสูตรหรือแผนการเรียน..."
							/>
							<Command.List class="max-h-64">
								{#if filteredPrograms.length === 0}<Command.Empty>ไม่พบแผนการเรียน</Command.Empty
									>{:else}<Command.Group>
										{#each filteredPrograms as program (program.id)}
											<Command.Item
												value={`${program.curriculumName} ${program.code} ${program.name}`}
												onSelect={() => toggleProgram(program.id)}
											>
												<Checkbox
													checked={studyProgramIds.includes(program.id)}
													class="pointer-events-none"
													aria-label={`เลือก ${program.name}`}
												/>
												<div class="min-w-0">
													<p class="truncate">{program.name}</p>
													<p class="truncate text-xs text-muted-foreground">
														{program.curriculumName} · {program.code}
													</p>
												</div>
											</Command.Item>
										{/each}
									</Command.Group>{/if}
							</Command.List>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
			{/if}
		</div>
	</div>
	<p class="rounded-lg bg-muted/45 px-3 py-2 text-xs leading-relaxed text-muted-foreground">
		แต่ละรายการจะรับกลุ่มสาระหรือสังกัดกิจกรรมพัฒนาผู้เรียนจากทะเบียนโดยอัตโนมัติ
	</p>
	<div class="flex justify-end">
		<LoadingButton
			type="button"
			variant="outline"
			loading={busy}
			loadingLabel="กำลังตรวจ"
			disabled={studyProgramIds.length === 0}
			onclick={buildPreview}><Eye class="size-4" /> ตรวจและจัดกลุ่มก่อน</LoadingButton
		>
	</div>

	{#if preview}
		<div class="overflow-hidden rounded-xl border">
			<div class="flex items-center justify-between gap-3 border-b bg-muted/30 px-4 py-3">
				<div>
					<h3 class="font-medium">ผลตรวจจากหลักสูตร</h3>
					<p class="text-xs text-muted-foreground">
						{displayedProposals.length} รายการเปิดสอน · ยังไม่บันทึก
					</p>
				</div>
				<Badge variant="secondary">ตรวจทานกลุ่มก่อนยืนยัน</Badge>
			</div>
			<div class="max-h-[32rem] divide-y overflow-auto">
				{#each displayedProposals as proposal (proposal.proposalId)}
					{@const choice = choiceFor(proposal.proposalId)}
					<div class="space-y-3 p-4">
						<div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
							<div class="min-w-0">
								<div class="flex flex-wrap items-center gap-2">
									<Badge variant={proposal.offeringAction === 'create' ? 'default' : 'secondary'}
										>{proposal.offeringAction === 'create' ? 'สร้างรายการ' : 'ใช้รายการเดิม'}</Badge
									>
									<p class="font-medium">{proposal.code} · {proposal.name}</p>
								</div>
								<p class="mt-1 text-xs text-muted-foreground">
									เป้าหมาย {proposal.targetHomeroomIds.length} ห้อง · ข้อกำหนด {proposal
										.requirementIds.length} รายการ
								</p>
							</div>
							{#if choice}
								<Select.Root
									type="single"
									value={choice.action}
									onValueChange={(value) =>
										setAction(proposal, value as CurriculumPreparationChoice['action'])}
								>
									<Select.Trigger class="w-full sm:w-52" aria-label={`วิธีเตรียม ${proposal.name}`}
										>{choice.action === 'apply'
											? 'เปิดสอนและจัดกลุ่ม'
											: choice.action === 'defer_groups'
												? 'เปิดสอน รอจัดกลุ่ม'
												: 'ข้ามรายการนี้'}</Select.Trigger
									>
									<Select.Content>
										{#if proposal.conflicts.length === 0}<Select.Item value="apply"
												>เปิดสอนและจัดกลุ่ม</Select.Item
											>{/if}
										<Select.Item value="defer_groups">เปิดสอน รอจัดกลุ่ม</Select.Item>
										{#if !preparationTarget}<Select.Item value="skip">ข้ามรายการนี้</Select.Item
											>{/if}
									</Select.Content>
								</Select.Root>
							{/if}
						</div>

						{#each proposal.conflicts as conflict (conflict.code)}
							<div
								class="flex gap-2 rounded-lg border border-amber-500/30 bg-amber-500/[0.06] p-3 text-sm"
							>
								<CircleAlert class="mt-0.5 size-4 shrink-0 text-amber-700" /><span
									>{conflict.message}</span
								>
							</div>
						{/each}

						{#if choice?.action === 'apply'}
							<div class="rounded-lg border bg-muted/[0.12] p-3">
								<div class="flex flex-wrap items-center justify-between gap-2">
									<p class="text-sm font-medium">
										กลุ่มที่จะสร้างหรือใช้ต่อ {choice.groups.length} กลุ่ม
									</p>
									<div class="flex flex-wrap gap-1.5">
										<Button
											type="button"
											size="sm"
											variant="ghost"
											onclick={() => restoreDefaults(proposal)}
											><RotateCcw class="size-3.5" /> คืนค่าข้อเสนอ</Button
										>
										{#if proposal.targetHomeroomIds.length > 1}<Button
												type="button"
												size="sm"
												variant="ghost"
												onclick={() => combineGroups(proposal)}
												><Combine class="size-3.5" /> รวมหลายห้อง</Button
											>{/if}
										<Button
											type="button"
											size="sm"
											variant="ghost"
											onclick={() => addSplitGroup(proposal)}
											><Plus class="size-3.5" /> เพิ่มกลุ่มแบ่ง</Button
										>
									</div>
								</div>
								<div class="mt-2 grid gap-2">
									{#each choice.groups as group (group.groupKey)}
										<div
											class="grid gap-2 rounded-lg border bg-background p-2 sm:grid-cols-[minmax(180px,1fr)_minmax(180px,0.8fr)_auto] sm:items-center"
										>
											<Input
												value={group.name}
												aria-label="ชื่อกลุ่มเรียน"
												oninput={(event) =>
													updateGroupName(
														proposal.proposalId,
														group.groupKey,
														event.currentTarget.value
													)}
											/>
											<Popover.Root>
												<Popover.Trigger>
													{#snippet child({ props })}
														<Button
															type="button"
															variant="outline"
															class="w-full justify-between font-normal"
															aria-label={`เลือกห้องสำหรับ ${group.name}`}
															{...props}
														>
															<span class="truncate"
																>{group.homeroomIds.length > 0
																	? groupRooms(group)
																	: 'เลือกห้อง'}</span
															><ChevronsUpDown class="size-4 opacity-50" />
														</Button>
													{/snippet}
												</Popover.Trigger>
												<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
													<Command.Root>
														<Command.List class="max-h-52">
															<Command.Group heading="ห้องเป้าหมายจากหลักสูตร">
																{#each proposal.targetHomeroomIds as homeroomId (homeroomId)}
																	<Command.Item
																		value={homeroomLabel(homeroomId)}
																		onSelect={() =>
																			toggleGroupHomeroom(
																				proposal.proposalId,
																				group.groupKey,
																				homeroomId
																			)}
																	>
																		<Checkbox
																			checked={group.homeroomIds.includes(homeroomId)}
																			class="pointer-events-none"
																			aria-label={`เลือก ${homeroomLabel(homeroomId)}`}
																		/>
																		{homeroomLabel(homeroomId)}
																	</Command.Item>
																{/each}
															</Command.Group>
														</Command.List>
													</Command.Root>
												</Popover.Content>
											</Popover.Root>
											<Button
												type="button"
												size="icon"
												variant="ghost"
												aria-label={`ลบ ${group.name}`}
												onclick={() => removeGroup(proposal.proposalId, group.groupKey)}
												><Trash2 class="size-4" /></Button
											>
										</div>
									{/each}
								</div>
							</div>
						{:else if choice?.action === 'defer_groups'}
							<p class="rounded-lg border border-dashed p-3 text-sm text-muted-foreground">
								สร้างหรือคงรายการเปิดสอนไว้ก่อน โดยยังไม่เดากลุ่ม ครู นักเรียน ห้องจริง หรือตารางสอน
							</p>
						{/if}
					</div>
				{/each}
			</div>
			<div class="flex justify-end border-t p-3">
				<LoadingButton
					type="button"
					loading={busy}
					loadingLabel="กำลังนำมาใช้"
					disabled={applyBlocked}
					onclick={applyPreview}
					><WandSparkles class="size-4" />
					{preparationTarget
						? 'เปิดใช้งานกิจกรรม'
						: 'ยืนยันรายการและกลุ่มที่ตรวจแล้ว'}</LoadingButton
				>
			</div>
		</div>
	{/if}
	{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
</div>
