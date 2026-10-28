<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		buildLearningDeliveryChangeRows,
		groupLearningDeliveryChanges,
		filterLearningDeliveryChanges,
		deliveryChangePage
	} from '#lib/academic/learning-delivery-change-list.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { deliveryOfferingLabel } from '#lib/academic/learning-delivery-presentation.js';
	import { deliveryOfferingHref } from '#lib/navigation/learning-delivery.js';
	import type { LearningDeliveryRefreshScope } from '#lib/academic/learning-delivery-page.js';
	import {
		deleteAcademicTermChangeItem,
		getAcademicTermChangeSet,
		getLearningDeliveryManagementOptions,
		type AcademicTermChangeSet,
		type ApplyTeacherHandoffResponse,
		type DeliveryManagementOptions,
		type LearningTeacherRole,
		type DeliveryVersion
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { ExternalLink, Trash2 } from '@lucide/svelte';
	import AcademicChangeReadiness from './AcademicChangeReadiness.svelte';
	import TeacherHandoffPanel from './TeacherHandoffPanel.svelte';
	type ChangeItem = AcademicTermChangeSet['items'][number];
	type StopTeacherItem = Extract<ChangeItem, { actionKind: 'stop_group_teacher' }>;
	let {
		changeSet,
		offerings,
		canManage,
		returnTo = '',
		initialTeacherChangeItemId = '',
		onChanged,
		onDeleted,
		ensureOfferings
	}: {
		changeSet: AcademicTermChangeSet;
		offerings: DeliveryVersion['snapshot']['offerings'];
		canManage: boolean;
		returnTo?: string;
		initialTeacherChangeItemId?: string;
		onChanged: (
			changeSet: AcademicTermChangeSet,
			refreshScope?: LearningDeliveryRefreshScope
		) => void | Promise<void>;
		onDeleted: (sourceVersionId: string | null) => Promise<void>;
		ensureOfferings: () => Promise<void>;
	} = $props();
	let changeRows = $derived(buildLearningDeliveryChangeRows(changeSet.changes, changeSet.items));
	let detailsOpen = $state(false);
	let openChangeGroupIds = $state<string[]>([]);
	let changeSearch = $state('');
	let changePage = $state(0);
	let rowPages = $state<Record<string, number>>({});
	let entryPages = $state<Record<string, number>>({});
	let groupedChanges = $derived(
		groupLearningDeliveryChanges(changeRows, changeSet.offeringLabels, offerings)
	);
	let filteredChanges = $derived(filterLearningDeliveryChanges(groupedChanges, changeSearch));
	let visibleChanges = $derived(deliveryChangePage(filteredChanges, changePage));
	let deletingItemId = $state('');
	let readinessRevision = $state(0);
	let errorMessage = $state('');
	let managementOptions = $state.raw<DeliveryManagementOptions | null>(null);
	let loadingOptions = $state(false);
	let handoffItemId = $state('');
	let handoffOpen = $state(false);
	const optionsRequest = new LatestRequest();
	let activeHandoffItem = $derived(
		changeSet.items.find(
			(item): item is StopTeacherItem =>
				item.id === handoffItemId && item.actionKind === 'stop_group_teacher'
		) ?? null
	);
	function searchChanges(value: string) {
		changeSearch = value;
		changePage = 0;
		rowPages = {};
		entryPages = {};
	}
	function formatDate(value: string | null): string {
		return value
			? new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
					new Date(`${value}T00:00:00`)
				)
			: 'เลือกวันเริ่มใช้ตอนเผยแพร่';
	}
	async function showChanges() {
		detailsOpen = true;
		await ensureOfferings();
	}
	async function loadManagementOptions() {
		if (managementOptions || loadingOptions) return;
		const ticket = optionsRequest.begin();
		loadingOptions = true;
		errorMessage = '';
		try {
			const loaded = await getLearningDeliveryManagementOptions(changeSet.academicTermId, {
				signal: ticket.signal
			});
			if (optionsRequest.isCurrent(ticket.revision)) managementOptions = loaded;
		} catch (error) {
			if (!isAbortError(error) && optionsRequest.isCurrent(ticket.revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (optionsRequest.isCurrent(ticket.revision)) loadingOptions = false;
		}
	}
	async function showHandoff(itemId: string) {
		if (
			!canManage ||
			changeSet.status !== 'published' ||
			!$can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)
		)
			return;
		detailsOpen = false;
		handoffItemId = itemId;
		handoffOpen = true;
		await loadManagementOptions();
	}
	function teacherRoleLabel(role: LearningTeacherRole): string {
		return role === 'primary' ? 'ครูหลัก' : role === 'secondary' ? 'ครูร่วม' : 'ครูผู้ช่วย';
	}
	function itemTitle(item: ChangeItem): string {
		switch (item.actionKind) {
			case 'add_offering':
				return 'เพิ่มรายการเปิดสอน';
			case 'stop_offering':
				return 'หยุดรายการเปิดสอน';
			case 'adjust_weekly_period_target':
				return 'ปรับจำนวนคาบต่อสัปดาห์';
			case 'add_group_teacher':
				return 'เพิ่มครูในกลุ่มเรียน';
			case 'adjust_group_teacher_role':
				return 'ปรับบทบาทครู';
			case 'stop_group_teacher':
				return 'หยุดความรับผิดชอบของครู';
		}
	}

	function itemDescription(item: ChangeItem): string {
		if (
			item.actionKind === 'add_group_teacher' ||
			item.actionKind === 'adjust_group_teacher_role' ||
			item.actionKind === 'stop_group_teacher'
		) {
			const role =
				item.actionKind === 'stop_group_teacher' ? '' : ` · ${teacherRoleLabel(item.teacherRole)}`;
			return `${item.learningGroupLabel} · ${item.teacherLabel}${role}`;
		}
		const offering = changeSet.offeringLabels.find((entry) => entry.id === item.learningOfferingId);
		const snapshotOffering = offerings.find((entry) => entry.id === item.learningOfferingId);
		const periods =
			item.actionKind === 'add_offering' || item.actionKind === 'adjust_weekly_period_target'
				? ` · ${item.weeklyPeriodTarget} คาบ/สัปดาห์`
				: '';
		return `${snapshotOffering ? deliveryOfferingLabel(snapshotOffering) : (offering?.name ?? 'ชื่อรายการไม่พร้อม กรุณาโหลดล่าสุด')}${periods}`;
	}

	async function recoverItemConflict(message: string) {
		readinessRevision += 1;
		errorMessage = message;
		try {
			await onChanged(await getAcademicTermChangeSet(changeSet.id));
		} catch (error) {
			errorMessage = error instanceof Error ? `${message} (${error.message})` : message;
		}
	}
	async function handoffApplied(_result: ApplyTeacherHandoffResponse) {
		await onChanged(changeSet, 'homerooms');
		readinessRevision += 1;
	}
	onMount(() => {
		if (initialTeacherChangeItemId) void showHandoff(initialTeacherChangeItemId);
	});
	onDestroy(() => optionsRequest.abort());
	async function removeItem(itemId: string, itemRowVersion: number) {
		if (!canManage) return;
		deletingItemId = itemId;
		errorMessage = '';
		try {
			const updated = await deleteAcademicTermChangeItem(changeSet.id, itemId, {
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion
			});
			await onChanged(updated);
			readinessRevision += 1;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverItemConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ลบรายการเปลี่ยนแปลงไม่สำเร็จ';
		} finally {
			deletingItemId = '';
		}
	}
</script>

<section
	class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3"
	aria-label="สรุปรุ่นเปิดสอน"
>
	<div class="min-w-0">
		<div class="flex flex-wrap items-center gap-2">
			<Badge variant="outline"
				>{changeSet.status === 'draft'
					? 'แบบร่าง'
					: changeSet.status === 'published'
						? 'เผยแพร่แล้ว'
						: 'ยกเลิกแล้ว'}</Badge
			>
			<p class="text-sm font-medium">{changeSet.reason}</p>
		</div>
		<p class="mt-1 text-xs text-muted-foreground">{formatDate(changeSet.effectiveFrom)}</p>
	</div>
	<div class="flex flex-wrap items-center gap-2">
		<Button size="sm" variant="outline" onclick={showChanges}>ดูรายละเอียดการเปลี่ยนแปลง</Button>
		{#key `${changeSet.id}:${readinessRevision}`}<AcademicChangeReadiness
				{changeSet}
				{canManage}
				{returnTo}
				{onChanged}
				{onDeleted}
			/>{/key}
	</div>
</section>
<Dialog.Root bind:open={detailsOpen}>
	<Dialog.Content class="flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden sm:max-w-4xl">
		<Dialog.Header class="shrink-0"
			><Dialog.Title>รายละเอียดการเปลี่ยนแปลง</Dialog.Title><Dialog.Description
				>{changeSet.reason} · การเปลี่ยนแปลงมีผลเฉพาะรุ่นเปิดสอนนี้</Dialog.Description
			></Dialog.Header
		>
		<div class="min-h-0 space-y-3 overflow-y-auto">
			<p class="text-sm text-muted-foreground" data-testid="delivery-change-summary">
				{groupedChanges.length} กลุ่มรายการ · {changeRows.length} รายการย่อย
			</p>
			<Label for="delivery-change-search">ค้นหารายวิชา กิจกรรม กลุ่ม หรือครู</Label>
			<Input
				id="delivery-change-search"
				value={changeSearch}
				oninput={(event) => searchChanges(event.currentTarget.value)}
				placeholder="ค้นหารายการเปลี่ยนแปลง..."
			/>
			<div class="space-y-3" aria-label="ความเปลี่ยนแปลงจากรุ่นต้นทาง">
				{#each visibleChanges.items as group (group.id)}
					<Collapsible.Root
						class="rounded-xl border"
						data-testid="delivery-change-offering"
						open={openChangeGroupIds.includes(group.id)}
						onOpenChange={(open) =>
							(openChangeGroupIds = open
								? [...openChangeGroupIds, group.id]
								: openChangeGroupIds.filter((id) => id !== group.id))}
					>
						<Collapsible.Trigger>
							{#snippet child({ props })}<Button
									variant="ghost"
									class="h-auto w-full justify-between gap-3 p-3 text-start whitespace-normal"
									{...props}
								>
									<span class="min-w-0 break-words">{group.label}</span><span
										class="shrink-0 text-xs text-muted-foreground"
										>{group.rows.length} รายการ · ดูรายละเอียด</span
									>
								</Button>{/snippet}
						</Collapsible.Trigger>
						<Collapsible.Content class="space-y-3 border-t p-3">
							{#if openChangeGroupIds.includes(group.id)}
								{@const rows = deliveryChangePage(group.rows, rowPages[group.id] ?? 0)}
								{#each rows.items as row (row.resourceId)}
									{@const firstChange = row.changes[0]}
									{@const entries = deliveryChangePage(
										[
											...row.changes.map((change) => ({ kind: 'change' as const, change })),
											...row.items.map((item) => ({ kind: 'item' as const, item }))
										],
										entryPages[row.resourceId] ?? 0
									)}
									<div class="rounded-xl border p-3">
										<div class="flex flex-wrap items-start justify-between gap-2">
											<div class="min-w-0 flex-1 space-y-1">
												{#if firstChange}
													<p class="font-medium">
														{firstChange.kind === 'added'
															? 'เพิ่ม'
															: firstChange.kind === 'removed'
																? 'ลบออกจากรุ่น'
																: 'เปลี่ยน'}
														{firstChange.label}
													</p>
												{/if}
												{#each entries.items as entry, index (`${entry.kind}:${index}`)}
													{#if entry.kind === 'change'}
														<p class="text-sm break-words text-muted-foreground">
															{#if entry.change.label !== firstChange?.label}{entry.change.label} ·
															{/if}
															{entry.change.field}: {entry.change.before ?? 'ยังไม่มี'} → {entry
																.change.after ?? 'ไม่ใช้ในรุ่นใหม่'}
														</p>
													{:else}
														<p class="text-sm font-medium">{itemTitle(entry.item)}</p>
														<p class="text-sm break-words text-muted-foreground">
															{itemDescription(entry.item)}
														</p>
													{/if}
												{/each}
											</div>
											<div class="flex max-w-full flex-wrap items-center gap-1">
												{#each entries.items.flatMap( (entry) => (entry.kind === 'item' ? [entry.item] : []) ) as item (item.id)}
													{#if item.actionKind === 'add_offering'}
														<Button
															href={deliveryOfferingHref(
																item.learningOfferingId,
																{
																	academicYearId: changeSet.academicYearId,
																	academicTermId: changeSet.academicTermId,
																	deliveryVersionId: changeSet.targetDeliveryVersionId
																},
																returnTo
															)}
															data-sveltekit-preload-data="tap"
															size="sm"
															variant="ghost"
														>
															{changeSet.status === 'draft' ? 'จัดกลุ่มและครู' : 'ดูรายละเอียด'}
															<ExternalLink class="size-3.5" />
														</Button>
													{/if}
													{#if item.actionKind === 'stop_group_teacher' && changeSet.status === 'published' && canManage && $can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)}
														<Button size="sm" variant="ghost" onclick={() => showHandoff(item.id)}>
															จัดการคาบที่ได้รับผลกระทบ
															<ExternalLink class="size-3.5" />
														</Button>
													{/if}
													{#if canManage && changeSet.status === 'draft'}
														<Button
															size={row.items.length > 1 ? 'sm' : 'icon'}
															variant="ghost"
															disabled={deletingItemId !== ''}
															onclick={() => removeItem(item.id, item.rowVersion)}
															aria-label={`ลบ ${itemTitle(item)} · ${itemDescription(item)}`}
															title={`ลบ ${itemTitle(item)} · ${itemDescription(item)}`}
														>
															<Trash2 class="size-4 shrink-0" />
															{#if row.items.length > 1}
																<span class="text-wrap text-left">
																	{itemTitle(item)}{'teacherLabel' in item
																		? ` · ${item.teacherLabel}`
																		: ''}
																</span>
															{/if}
														</Button>
													{/if}
												{/each}
											</div>
										</div>
										{#if entries.pageCount > 1}
											<div
												class="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
												aria-label="หน้ารายละเอียดการเปลี่ยนแปลง"
											>
												<Button
													size="sm"
													variant="outline"
													disabled={entries.page === 0}
													onclick={() => (entryPages[row.resourceId] = entries.page - 1)}
													>รายละเอียดก่อนหน้า</Button
												>
												<span>{entries.page + 1}/{entries.pageCount}</span>
												<Button
													size="sm"
													variant="outline"
													disabled={entries.page + 1 === entries.pageCount}
													onclick={() => (entryPages[row.resourceId] = entries.page + 1)}
													>รายละเอียดถัดไป</Button
												>
											</div>
										{/if}
									</div>
								{/each}
								{#if rows.pageCount > 1}
									<div
										class="flex flex-wrap items-center justify-end gap-2 text-sm"
										aria-label={`หน้ากลุ่ม ${group.label}`}
									>
										<Button
											size="sm"
											variant="outline"
											disabled={rows.page === 0}
											onclick={() => (rowPages[group.id] = rows.page - 1)}>กลุ่มก่อนหน้า</Button
										><span>{rows.page + 1}/{rows.pageCount}</span><Button
											size="sm"
											variant="outline"
											disabled={rows.page + 1 === rows.pageCount}
											onclick={() => (rowPages[group.id] = rows.page + 1)}>กลุ่มถัดไป</Button
										>
									</div>
								{/if}
							{/if}
						</Collapsible.Content>
					</Collapsible.Root>
				{:else}
					<p class="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
						{changeSearch
							? 'ไม่พบรายการที่ตรงกับการค้นหา'
							: 'ยังไม่มีรายการเพิ่ม ปรับ หยุด หรือเปลี่ยนครู'}
					</p>
				{/each}
			</div>
			{#if visibleChanges.pageCount > 1}
				<div
					class="flex flex-wrap items-center justify-end gap-2 text-sm"
					aria-label="หน้ารายการเปิดสอนที่เปลี่ยน"
				>
					<Button
						size="sm"
						variant="outline"
						disabled={visibleChanges.page === 0}
						onclick={() => (changePage = visibleChanges.page - 1)}>รายการก่อนหน้า</Button
					><span
						>{visibleChanges.page + 1}/{visibleChanges.pageCount} · {visibleChanges.total} รายการ</span
					><Button
						size="sm"
						variant="outline"
						disabled={visibleChanges.page + 1 === visibleChanges.pageCount}
						onclick={() => (changePage = visibleChanges.page + 1)}>รายการถัดไป</Button
					>
				</div>
			{/if}
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
		<Dialog.Footer class="shrink-0"
			><Button variant="outline" onclick={() => (detailsOpen = false)}>ปิดรายละเอียด</Button
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
<Dialog.Root bind:open={handoffOpen}>
	<Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header
			><Dialog.Title>จัดการคาบที่ได้รับผลกระทบ</Dialog.Title><Dialog.Description
				>ส่งต่อครูสำหรับรายการเปลี่ยนแปลงที่เผยแพร่แล้ว</Dialog.Description
			></Dialog.Header
		>
		{#if loadingOptions}<p role="status">
				กำลังโหลดตัวเลือก
			</p>{:else if activeHandoffItem && managementOptions && canManage && changeSet.status === 'published' && $can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)}
			{#key activeHandoffItem.id}<TeacherHandoffPanel
					{changeSet}
					teacherChangeItem={activeHandoffItem}
					{managementOptions}
					{onChanged}
					onApplied={handoffApplied}
					onClose={() => (handoffOpen = false)}
				/>{/key}
		{:else if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
			<Button variant="outline" onclick={loadManagementOptions}>ลองอีกครั้ง</Button>{/if}
	</Dialog.Content>
</Dialog.Root>
