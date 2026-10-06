<script lang="ts">
	import { onMount } from 'svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import type { LearningDeliveryRefreshScope } from '#lib/academic/learning-delivery-page.js';
	import {
		deleteAcademicTermChangeItem,
		getAcademicTermChangeSet,
		getLearningDeliveryManagementOptions,
		upsertAcademicTermChangeItem,
		type AcademicTermChangeSet,
		type ApplyTeacherHandoffResponse,
		type DeliveryManagementOptions,
		type LearningTeacherRole,
		type DeliveryVersion,
		type UpsertAcademicTermChangeItemRequest
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import {
		ArrowRight,
		CalendarClock,
		ExternalLink,
		Plus,
		Trash2,
		UserRoundPlus,
		X
	} from '@lucide/svelte';
	import AcademicChangeReadiness from './AcademicChangeReadiness.svelte';
	import AcademicTeacherChangeForm from './AcademicTeacherChangeForm.svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';
	import TeacherHandoffPanel from './TeacherHandoffPanel.svelte';

	type ChangeAction =
		'add_course' | 'add_activity' | 'stop_offering' | 'adjust_weekly_period_target';
	type ChangeItem = AcademicTermChangeSet['items'][number];
	type StopTeacherItem = Extract<ChangeItem, { actionKind: 'stop_group_teacher' }>;

	let {
		changeSet,
		offerings,
		canManage,
		initialTeacherChangeItemId = '',
		onChanged,
		onDeleted,
		ensureOfferings
	}: {
		changeSet: AcademicTermChangeSet;
		offerings: DeliveryVersion['snapshot']['offerings'];
		canManage: boolean;
		initialTeacherChangeItemId?: string;
		onChanged: (
			changeSet: AcademicTermChangeSet,
			refreshScope?: LearningDeliveryRefreshScope
		) => void | Promise<void>;
		onDeleted: (sourceVersionId: string | null) => Promise<void>;
		ensureOfferings: () => Promise<void>;
	} = $props();

	let managementOptions = $state.raw<DeliveryManagementOptions | null>(null);
	let loadingOptions = $state(false);
	let savingItem = $state(false);
	let deletingItemId = $state('');
	let readinessRevision = $state(0);
	let itemFormOpen = $state(false);
	let teacherFormOpen = $state(false);
	let handoffItemId = $state('');
	let action = $state<ChangeAction>('add_course');
	let catalogVersionId = $state('');
	let gradeLevelId = $state('');
	let studyProgramId = $state('');
	let learningOfferingId = $state('');
	let weeklyPeriodTarget = $state(1);
	let errorMessage = $state('');
	let selectedCatalogVersion = $derived(
		managementOptions?.catalogVersions.find((item) => item.id === catalogVersionId) ?? null
	);
	let selectedOffering = $derived(offerings.find((item) => item.id === learningOfferingId) ?? null);
	$effect(() => {
		if (action === 'adjust_weekly_period_target' && selectedOffering) {
			weeklyPeriodTarget = selectedOffering.weeklyPeriodTarget;
		}
	});
	let catalogOptions = $derived(
		(managementOptions?.catalogVersions ?? [])
			.filter((item) => item.kind === (action === 'add_activity' ? 'activity' : 'course'))
			.sort(compareOfferingOptions)
			.map((item) => ({
				id: item.id,
				label: item.kind === 'activity' ? item.name : item.label,
				description:
					item.kind === 'course' && item.standardPeriodsPerWeek
						? `มาตรฐาน ${item.standardPeriodsPerWeek} คาบ/สัปดาห์`
						: item.kind === 'activity'
							? `กิจกรรมพัฒนาผู้เรียน · ฉบับ ${item.versionNo}`
							: undefined
			}))
	);
	let offeringOptions = $derived(
		offerings
			.filter(
				(item) =>
					!changeSet.items.some(
						(change) =>
							change.actionKind === 'add_offering' && change.learningOfferingId === item.id
					)
			)
			.sort(compareOfferingOptions)
			.map((item) => ({
				id: item.id,
				label: item.kind === 'activity' ? item.name : `${item.code} — ${item.name}`,
				description: item.kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน'
			}))
	);

	function compareOfferingOptions(
		left: { kind: 'course' | 'activity'; code: string; name: string },
		right: { kind: 'course' | 'activity'; code: string; name: string }
	): number {
		if (left.kind !== right.kind) return left.kind === 'course' ? -1 : 1;
		const leftLabel = left.kind === 'course' ? left.code : left.name;
		const rightLabel = right.kind === 'course' ? right.code : right.name;
		return (
			leftLabel.localeCompare(rightLabel, 'th-TH', { numeric: true }) ||
			left.name.localeCompare(right.name, 'th-TH')
		);
	}
	let activeHandoffItem = $derived(
		changeSet.items.find(
			(item): item is StopTeacherItem =>
				item.id === handoffItemId && item.actionKind === 'stop_group_teacher'
		) ?? null
	);

	function formatDate(value: string | null): string {
		if (!value) return 'เลือกวันเริ่มใช้ตอนเผยแพร่';
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function resetItemForm(nextAction: ChangeAction = action) {
		action = nextAction;
		catalogVersionId = '';
		gradeLevelId = '';
		studyProgramId = '';
		learningOfferingId = '';
		weeklyPeriodTarget = 1;
		errorMessage = '';
	}

	async function loadManagementOptions(): Promise<boolean> {
		if (managementOptions) return true;
		if (loadingOptions) return false;
		loadingOptions = true;
		try {
			managementOptions = await getLearningDeliveryManagementOptions(changeSet.academicTermId);
			return true;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
			return false;
		} finally {
			loadingOptions = false;
		}
	}

	async function showItemForm() {
		if (!canManage || changeSet.status !== 'draft') return;
		teacherFormOpen = false;
		handoffItemId = '';
		itemFormOpen = true;
		await Promise.all([loadManagementOptions(), ensureOfferings()]);
	}

	async function showTeacherForm() {
		if (!canManage || changeSet.status !== 'draft') return;
		itemFormOpen = false;
		handoffItemId = '';
		teacherFormOpen = true;
		await loadManagementOptions();
	}

	async function showHandoff(itemId: string) {
		if (
			!canManage ||
			changeSet.status !== 'published' ||
			!$can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)
		)
			return;
		itemFormOpen = false;
		teacherFormOpen = false;
		if (!(await loadManagementOptions())) return;
		handoffItemId = itemId;
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
		const periods =
			item.actionKind === 'add_offering' || item.actionKind === 'adjust_weekly_period_target'
				? ` · ${item.weeklyPeriodTarget} คาบ/สัปดาห์`
				: '';
		return `${offering ? `${offering.code} — ${offering.name}` : 'ชื่อรายการไม่พร้อม กรุณาโหลดล่าสุด'}${periods}`;
	}

	async function teacherItemSaved(updated: AcademicTermChangeSet) {
		await onChanged(updated);
		teacherFormOpen = false;
		readinessRevision += 1;
	}

	async function handoffApplied(_result: ApplyTeacherHandoffResponse) {
		await onChanged(changeSet, 'homerooms');
		readinessRevision += 1;
	}

	onMount(() => {
		handoffItemId = initialTeacherChangeItemId;
		if (
			handoffItemId &&
			changeSet.status === 'published' &&
			canManage &&
			$can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)
		) {
			void loadManagementOptions();
		}
	});

	async function recoverItemConflict(message: string) {
		readinessRevision += 1;
		errorMessage = message;
		try {
			const current = await getAcademicTermChangeSet(changeSet.id);
			await onChanged(current);
		} catch (error) {
			errorMessage =
				error instanceof Error
					? `${message} (${error.message})`
					: `${message} และโหลดข้อมูลล่าสุดไม่สำเร็จ`;
		}
	}

	function targetInput() {
		return [
			{
				targetKind: 'grade_program' as const,
				homeroomId: null,
				gradeLevelId,
				studyProgramId
			}
		];
	}

	function itemRequest(): UpsertAcademicTermChangeItemRequest | null {
		if (action === 'stop_offering') {
			if (!learningOfferingId) return null;
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion: null,
				learningOfferingId
			};
		}
		if (action === 'adjust_weekly_period_target') {
			if (
				!selectedOffering ||
				!Number.isInteger(weeklyPeriodTarget) ||
				weeklyPeriodTarget < (selectedOffering.kind === 'course' ? 0 : 1)
			)
				return null;
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion: null,
				learningOfferingId,
				weeklyPeriodTarget
			};
		}
		if (!catalogVersionId || !gradeLevelId || !studyProgramId) return null;
		if (action === 'add_course') {
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				offering: {
					academicTermId: changeSet.academicTermId,
					subjectVersionId: catalogVersionId,
					curriculumCourseRequirementId: null,
					assessmentTotalScore: '100.00',
					targets: targetInput()
				}
			};
		}
		if (!Number.isInteger(weeklyPeriodTarget) || weeklyPeriodTarget <= 0) return null;
		return {
			action,
			changeSetRowVersion: changeSet.rowVersion,
			weeklyPeriodTarget,
			offering: {
				academicTermId: changeSet.academicTermId,
				activityVersionId: catalogVersionId,
				curriculumActivityRequirementId: null,
				registrationType: 'assigned',
				schedulingMode: 'synchronized',
				capacity: null,
				attendanceRequirement: { minimumPercent: '80.00', requiredSessions: null },
				passCriteria: {
					requireAttendance: true,
					requireTeacherConfirmation: true,
					outcomes: ['pass', 'fail']
				},
				targets: targetInput()
			}
		};
	}

	async function saveItem(event: SubmitEvent) {
		event.preventDefault();
		if (!canManage) return;
		const request = itemRequest();
		if (!request) return;
		savingItem = true;
		errorMessage = '';
		try {
			const updated = await upsertAcademicTermChangeItem(changeSet.id, request);
			await onChanged(updated);
			itemFormOpen = false;
			resetItemForm();
			readinessRevision += 1;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverItemConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'บันทึกรายการเปลี่ยนแปลงไม่สำเร็จ';
		} finally {
			savingItem = false;
		}
	}

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

<section class="overflow-hidden rounded-2xl border border-amber-500/30 bg-card">
	<header class="border-b border-amber-500/20 bg-amber-500/7 p-4 sm:p-5">
		<div class="flex flex-wrap items-start justify-between gap-4">
			<div class="flex min-w-0 items-start gap-3">
				<div class="rounded-xl bg-amber-500/15 p-2.5 text-amber-800">
					<CalendarClock class="size-5" />
				</div>
				<div class="min-w-0">
					<div class="flex flex-wrap items-center gap-2">
						<h2 class="font-semibold">รุ่นเปิดสอน</h2>
						<Badge variant="outline" class="border-amber-500/40 bg-background">
							{changeSet.status === 'draft'
								? 'แบบร่าง'
								: changeSet.status === 'published'
									? 'เผยแพร่แล้ว'
									: 'ยกเลิกแล้ว'}
						</Badge>
					</div>
					<p class="mt-1 text-sm text-muted-foreground">{changeSet.reason}</p>
				</div>
			</div>
			<div class="shrink-0 rounded-xl border bg-background px-4 py-2 text-end">
				<p class="text-xs text-muted-foreground">หลังเผยแพร่ เริ่มมีผล</p>
				<p class="font-medium text-amber-900">{formatDate(changeSet.effectiveFrom)}</p>
			</div>
		</div>
	</header>

	<div class="min-w-0 space-y-5 p-4 sm:p-5">
		<section class="space-y-3">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<div>
					<h3 class="font-medium">รายการที่จะเปลี่ยน</h3>
					<p class="text-xs text-muted-foreground">
						หลักสูตรไม่เปลี่ยน รายการเหล่านี้มีผลเฉพาะภาคเรียนนี้
					</p>
				</div>
				{#if canManage && changeSet.status === 'draft'}
					<div class="flex flex-wrap gap-2">
						<Button size="sm" variant="outline" onclick={showItemForm}>
							<Plus class="size-4" /> เพิ่ม/ปรับรายการสอน
						</Button>
						<Button size="sm" variant="outline" onclick={showTeacherForm}>
							<UserRoundPlus class="size-4" /> เปลี่ยนครูผู้สอน
						</Button>
					</div>
				{/if}
			</div>

			<div class="space-y-3" aria-label="ความเปลี่ยนแปลงจากรุ่นต้นทาง">
				{#each changeSet.changes as change, index (`${change.learningOfferingId}:${change.resourceId}:${change.field}:${index}`)}
					<div class="rounded-xl border p-3">
						<p class="font-medium">
							{change.kind === 'added' ? 'เพิ่ม' : change.kind === 'removed' ? 'หยุด' : 'เปลี่ยน'}
							{change.label}
						</p>
						<p class="text-sm text-muted-foreground">
							{change.field}: {change.before ?? 'ยังไม่มี'} → {change.after ?? 'ไม่ใช้ในรุ่นใหม่'}
						</p>
					</div>
				{:else}<p class="text-sm text-muted-foreground">
						ยังไม่มีข้อมูลที่ต่างจากรุ่นต้นทาง
					</p>{/each}
			</div>
			<p class="text-xs text-muted-foreground">รายการคำสั่งที่บันทึกไว้</p>
			{#if changeSet.items.length === 0}
				<div class="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
					ยังไม่มีรายการเพิ่ม ปรับ หยุด หรือเปลี่ยนครู
				</div>
			{:else}
				<div class="divide-y rounded-xl border">
					{#each changeSet.items as item (item.id)}
						<div class="flex items-center justify-between gap-3 p-3">
							<div class="min-w-0">
								<p class="font-medium">{itemTitle(item)}</p>
								<p class="truncate text-sm text-muted-foreground">{itemDescription(item)}</p>
							</div>
							<div class="flex shrink-0 gap-1">
								{#if item.actionKind === 'add_offering'}
									<Button
										href={`/staff/academic/delivery/${item.learningOfferingId}?deliveryVersionId=${changeSet.targetDeliveryVersionId}`}
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
										size="icon"
										variant="ghost"
										disabled={deletingItemId === item.id}
										onclick={() => removeItem(item.id, item.rowVersion)}
										aria-label="ลบรายการเปลี่ยนแปลง"
									>
										<Trash2 class="size-4" />
									</Button>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>

		{#if teacherFormOpen && canManage && changeSet.status === 'draft'}
			{#if loadingOptions || !managementOptions}
				<div class="h-48 animate-pulse rounded-xl bg-muted"></div>
			{:else}
				<AcademicTeacherChangeForm
					{changeSet}
					{managementOptions}
					onSaved={teacherItemSaved}
					onConflict={recoverItemConflict}
					onCancel={() => (teacherFormOpen = false)}
				/>
			{/if}
		{/if}

		{#if activeHandoffItem && managementOptions && canManage && changeSet.status === 'published' && $can.has(PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL)}
			{#key activeHandoffItem.id}
				<TeacherHandoffPanel
					{changeSet}
					teacherChangeItem={activeHandoffItem}
					{managementOptions}
					{onChanged}
					onApplied={handoffApplied}
					onClose={() => (handoffItemId = '')}
				/>
			{/key}
		{/if}

		{#if itemFormOpen && canManage && changeSet.status === 'draft'}
			<form
				class="space-y-4 rounded-xl border border-primary/20 bg-primary/[0.025] p-4"
				onsubmit={saveItem}
			>
				<div class="flex items-center justify-between gap-3">
					<div>
						<h3 class="font-medium">เพิ่มรายการเปลี่ยนแปลง</h3>
						<p class="text-xs text-muted-foreground">เลือกเฉพาะสิ่งที่เริ่มมีผลในวันที่กำหนด</p>
					</div>
					<Button
						type="button"
						size="icon"
						variant="ghost"
						onclick={() => (itemFormOpen = false)}
						aria-label="ปิดแบบฟอร์ม"
					>
						<X class="size-4" />
					</Button>
				</div>
				<div class="space-y-2">
					<Label>ประเภทการเปลี่ยนแปลง</Label>
					<Select.Root
						type="single"
						value={action}
						onValueChange={(value) => resetItemForm(value as ChangeAction)}
					>
						<Select.Trigger class="w-full">
							{action === 'add_course'
								? 'เพิ่มรายวิชา'
								: action === 'add_activity'
									? 'เพิ่มกิจกรรมพัฒนาผู้เรียน'
									: action === 'stop_offering'
										? 'หยุดรายการเปิดสอน'
										: 'ปรับคาบต่อสัปดาห์'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="add_course">เพิ่มรายวิชา</Select.Item>
							<Select.Item value="add_activity">เพิ่มกิจกรรมพัฒนาผู้เรียน</Select.Item>
							<Select.Item value="stop_offering">หยุดรายการเปิดสอน</Select.Item>
							<Select.Item value="adjust_weekly_period_target">ปรับคาบต่อสัปดาห์</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>

				{#if loadingOptions}
					<div class="h-28 animate-pulse rounded-xl bg-muted"></div>
				{:else if action === 'add_course' || action === 'add_activity'}
					{#if managementOptions}
						<div class="grid gap-4 sm:grid-cols-2">
							<div class="space-y-2 sm:col-span-2">
								<Label>{action === 'add_course' ? 'รายวิชา' : 'กิจกรรม'}</Label>
								<DeliveryOptionCombobox
									bind:value={catalogVersionId}
									options={catalogOptions}
									placeholder={action === 'add_course' ? 'เลือกรายวิชา' : 'เลือกกิจกรรม'}
									searchPlaceholder="ค้นหารหัสหรือชื่อ..."
								/>
							</div>
							<div class="space-y-2">
								<Label>ระดับชั้น</Label>
								<DeliveryOptionCombobox
									bind:value={gradeLevelId}
									options={managementOptions.gradeLevels.map((grade) => ({
										id: grade.id,
										label: grade.name,
										description: grade.short_name ?? grade.code
									}))}
									placeholder="เลือกระดับชั้น"
								/>
							</div>
							<div class="space-y-2">
								<Label>แผนการเรียน</Label>
								<DeliveryOptionCombobox
									bind:value={studyProgramId}
									options={managementOptions.studyPrograms.map((program) => ({
										id: program.id,
										label: program.name,
										description: `${program.curriculumName} · ${program.code}`
									}))}
									placeholder="เลือกแผนการเรียน"
								/>
							</div>
						</div>
						<p
							class="rounded-lg bg-muted/45 px-3 py-2 text-xs leading-relaxed text-muted-foreground"
						>
							ระบบจะใช้กลุ่มสาระหรือสังกัดกิจกรรมพัฒนาผู้เรียนจากทะเบียนโดยอัตโนมัติ
						</p>
						{#if action === 'add_course' && selectedCatalogVersion}
							<div
								class="grid gap-2 rounded-xl border bg-background p-3 sm:grid-cols-[1fr_auto_1fr] sm:items-center"
							>
								<div>
									<p class="text-xs text-muted-foreground">ตามหลักสูตร</p>
									<p class="font-semibold">
										{selectedCatalogVersion.standardPeriodsPerWeek ?? '—'} คาบ/สัปดาห์
									</p>
								</div>
								<ArrowRight class="hidden size-4 text-primary sm:block" />
								<div>
									<p class="text-xs text-muted-foreground">จัดจริงภาคเรียนนี้</p>
									<p class="font-semibold text-primary">
										เริ่มต้น {selectedCatalogVersion.standardPeriodsPerWeek ?? '—'} คาบ/สัปดาห์
									</p>
									<p class="text-[11px] text-muted-foreground">
										หากต้องการต่างจากมาตรฐาน ให้เพิ่มรายการ “ปรับคาบ” ต่อจากนี้
									</p>
								</div>
							</div>
						{:else if action === 'add_activity'}
							<div class="space-y-2">
								<Label for="activity-weekly-period-target">คาบที่จัดจริงภาคเรียนนี้</Label>
								<Input
									id="activity-weekly-period-target"
									type="number"
									min="1"
									bind:value={weeklyPeriodTarget}
								/>
								<p class="text-xs text-muted-foreground">
									กิจกรรมไม่มีค่าคาบมาตรฐาน จึงต้องกำหนดเป้าหมายก่อนจัดตาราง
								</p>
							</div>
						{/if}
					{/if}
				{:else}
					<div class="space-y-2">
						<Label>รายการเปิดสอน</Label>
						<DeliveryOptionCombobox
							bind:value={learningOfferingId}
							options={offeringOptions}
							placeholder="เลือกรายวิชาหรือกิจกรรม"
							searchPlaceholder="ค้นหารหัสหรือชื่อ..."
						/>
					</div>
					{#if action === 'adjust_weekly_period_target'}
						<div class="grid gap-2 rounded-xl border bg-background p-3 sm:grid-cols-2">
							<div>
								<p class="text-xs text-muted-foreground">ตามหลักสูตร</p>
								<p class="font-semibold">
									{selectedOffering?.catalog.kind === 'course'
										? selectedOffering.catalog.standardPeriodsPerWeek
										: 'ไม่มีค่ามาตรฐาน'}
									{selectedOffering?.catalog.kind === 'course' ? 'คาบ/สัปดาห์' : ''}
								</p>
							</div>
							<div class="space-y-1">
								<Label for="adjust-weekly-period-target">คาบที่จัดจริงต่อสัปดาห์</Label>
								<Input
									id="adjust-weekly-period-target"
									type="number"
									min={selectedOffering?.kind === 'course' ? 0 : 1}
									step="1"
									bind:value={weeklyPeriodTarget}
								/>
								{#if selectedOffering?.kind === 'course'}
									<p class="text-xs text-muted-foreground">
										0 คาบ = เปิดรายวิชาโดยไม่จัดคาบแยกในตารางสอน
										ใช้กับทุกกลุ่มเรียนของรายวิชานี้ในภาคเรียนนี้
										โดยหน่วยกิตและคาบตามหลักสูตรยังคงเดิม
									</p>
								{/if}
							</div>
						</div>
					{:else if selectedOffering}
						<p
							class="rounded-lg border border-rose-500/25 bg-rose-500/5 px-3 py-2 text-sm text-rose-800"
						>
							หยุดสอนตามวันเริ่มใช้ที่เลือกตอนเผยแพร่ โดยข้อมูลคะแนน ผลการเรียน
							และประวัติเดิมยังคงอยู่
						</p>
					{/if}
				{/if}

				{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
				<div class="flex justify-end gap-2">
					<Button type="button" variant="outline" onclick={() => (itemFormOpen = false)}
						>ยกเลิก</Button
					>
					<LoadingButton
						type="submit"
						loading={savingItem}
						loadingLabel="กำลังบันทึก"
						disabled={!itemRequest()}
					>
						บันทึกรายการ
					</LoadingButton>
				</div>
			</form>
		{/if}

		{#if errorMessage && !itemFormOpen}
			<p
				role="alert"
				class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-sm text-destructive"
			>
				{errorMessage}
			</p>
		{/if}

		{#key `${changeSet.id}:${readinessRevision}`}
			<AcademicChangeReadiness {changeSet} {canManage} {onChanged} {onDeleted} />
		{/key}
	</div>
</section>
