<script lang="ts">
	import {
		cancelAcademicTermChangeSet,
		deleteAcademicTermChangeItem,
		getAcademicTermChangeSet,
		getLearningDeliveryManagementOptions,
		previewAcademicTermChangeSet,
		publishAcademicTermChangeSet,
		upsertAcademicTermChangeItem,
		type AcademicChangeFinding,
		type AcademicChangeFindingCode,
		type AcademicTermChangeSet,
		type AcademicTermChangeSetPreview,
		type DeliveryManagementOptions,
		type LearningOfferingOverviewItem,
		type UpsertAcademicTermChangeItemRequest
	} from '$lib/api/learning-delivery';
	import { ApiClientError } from '$lib/api/client';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import {
		ArrowRight,
		CalendarClock,
		CheckCircle2,
		CircleAlert,
		Clock3,
		ExternalLink,
		Plus,
		RefreshCw,
		Send,
		Trash2,
		TriangleAlert,
		X
	} from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	type ChangeAction =
		| 'add_course'
		| 'add_activity'
		| 'stop_offering'
		| 'adjust_weekly_period_target';

	let {
		changeSet,
		offerings,
		canManage,
		onChanged
	}: {
		changeSet: AcademicTermChangeSet;
		offerings: LearningOfferingOverviewItem[];
		canManage: boolean;
		onChanged: (changeSet: AcademicTermChangeSet) => void | Promise<void>;
	} = $props();

	let preview = $state.raw<AcademicTermChangeSetPreview | null>(null);
	let managementOptions = $state.raw<DeliveryManagementOptions | null>(null);
	let loadingPreview = $state(false);
	let loadingOptions = $state(false);
	let savingItem = $state(false);
	let publishing = $state(false);
	let cancelling = $state(false);
	let deletingItemId = $state('');
	let itemFormOpen = $state(false);
	let action = $state<ChangeAction>('add_course');
	let catalogVersionId = $state('');
	let owningOrganizationUnitId = $state('');
	let gradeLevelId = $state('');
	let studyProgramId = $state('');
	let learningOfferingId = $state('');
	let weeklyPeriodTarget = $state(1);
	let acknowledgedWarnings = $state<AcademicChangeFindingCode[]>([]);
	let errorMessage = $state('');

	let blockingFindings = $derived(
		preview?.findings.filter((finding) => finding.severity === 'blocking') ?? []
	);
	let warningFindings = $derived(
		preview?.findings.filter((finding) => finding.severity === 'warning') ?? []
	);
	let warningsAcknowledged = $derived(
		warningFindings.every((finding) => acknowledgedWarnings.includes(finding.code))
	);
	let selectedCatalogVersion = $derived(
		managementOptions?.catalogVersions.find((item) => item.id === catalogVersionId) ?? null
	);
	let selectedOffering = $derived(
		offerings.find((item) => item.offering.id === learningOfferingId)?.offering ?? null
	);
	let catalogOptions = $derived(
		(managementOptions?.catalogVersions ?? [])
			.filter((item) => item.kind === (action === 'add_activity' ? 'activity' : 'course'))
			.map((item) => ({
				id: item.id,
				label: item.label,
				description:
					item.kind === 'course' && item.standardPeriodsPerWeek
						? `มาตรฐาน ${item.standardPeriodsPerWeek} คาบ/สัปดาห์`
						: item.kind === 'activity'
							? 'กิจกรรมพัฒนาผู้เรียน'
							: undefined
			}))
	);
	let offeringOptions = $derived(
		offerings
			.filter((item) => item.offering.status === 'published' && !item.offering.endsOn)
			.map((item) => ({
				id: item.offering.id,
				label: `${item.offering.codeSnapshot} — ${item.offering.nameSnapshot}`,
				description: item.offering.kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน'
			}))
	);

	const impactItems = $derived(
		preview
			? ([
					['กลุ่มเรียน', preview.impactCounts.groups],
					['ห้องประจำชั้น', preview.impactCounts.homerooms],
					['รายชื่อนักเรียน', preview.impactCounts.membershipIntervals],
					['ครูผู้สอน', preview.impactCounts.teacherAssignments],
					['คาบในตารางเป้าหมาย', preview.impactCounts.targetTimetableEntries],
					['แผนโครงสร้างคะแนน', preview.impactCounts.courseAssessmentPlans],
					['หมวดคะแนน', preview.impactCounts.courseAssessmentCategories],
					['รายการเก็บคะแนน', preview.impactCounts.courseAssessmentItems],
					['ผลการเรียน', preview.impactCounts.learningResults],
					['ตารางสอบ', preview.impactCounts.examScheduleItems],
					['นิเทศการสอน', preview.impactCounts.supervisionObservations]
				] as const)
			: []
	);

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function formatDateTime(value: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}

	function resetItemForm(nextAction: ChangeAction = action) {
		action = nextAction;
		catalogVersionId = '';
		owningOrganizationUnitId = '';
		gradeLevelId = '';
		studyProgramId = '';
		learningOfferingId = '';
		weeklyPeriodTarget = 1;
		errorMessage = '';
	}

	async function showItemForm() {
		if (!canManage || changeSet.status !== 'draft') return;
		itemFormOpen = true;
		if (managementOptions || loadingOptions) return;
		loadingOptions = true;
		try {
			managementOptions = await getLearningDeliveryManagementOptions(changeSet.academicTermId);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			loadingOptions = false;
		}
	}

	async function refreshPreview() {
		loadingPreview = true;
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = '';
		try {
			const current = await getAcademicTermChangeSet(changeSet.id);
			if (current.rowVersion !== changeSet.rowVersion) {
				await onChanged(current);
				return;
			}
			const loadedPreview = await previewAcademicTermChangeSet(changeSet.id);
			if (loadedPreview.changeSetRowVersion !== current.rowVersion) {
				await syncCurrentChangeSet();
				return;
			}
			preview = loadedPreview;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('ข้อมูลเปลี่ยนระหว่างตรวจ กรุณาตรวจความพร้อมใหม่อีกครั้ง');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ตรวจความพร้อมไม่สำเร็จ';
		} finally {
			loadingPreview = false;
		}
	}

	async function syncCurrentChangeSet() {
		const current = await getAcademicTermChangeSet(changeSet.id);
		await onChanged(current);
	}

	async function recoverFromConflict(message: string) {
		preview = null;
		acknowledgedWarnings = [];
		errorMessage = message;
		try {
			await syncCurrentChangeSet();
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
			if (!learningOfferingId || weeklyPeriodTarget <= 0) return null;
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion: null,
				learningOfferingId,
				weeklyPeriodTarget
			};
		}
		if (!catalogVersionId || !owningOrganizationUnitId || !gradeLevelId || !studyProgramId)
			return null;
		if (action === 'add_course') {
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				offering: {
					academicTermId: changeSet.academicTermId,
					subjectVersionId: catalogVersionId,
					curriculumCourseRequirementId: null,
					owningOrganizationUnitId,
					gradingPolicy: {
						policyCode: 'school_default',
						totalScore: '100.00',
						passingScore: '50.00'
					},
					targets: targetInput()
				}
			};
		}
		if (weeklyPeriodTarget <= 0) return null;
		return {
			action,
			changeSetRowVersion: changeSet.rowVersion,
			weeklyPeriodTarget,
			offering: {
				academicTermId: changeSet.academicTermId,
				activityVersionId: catalogVersionId,
				curriculumActivityRequirementId: null,
				owningOrganizationUnitId,
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
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
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
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ลบรายการเปลี่ยนแปลงไม่สำเร็จ';
		} finally {
			deletingItemId = '';
		}
	}

	function setWarningAcknowledgement(code: AcademicChangeFindingCode, checked: boolean) {
		acknowledgedWarnings = checked
			? [...new Set([...acknowledgedWarnings, code])]
			: acknowledgedWarnings.filter((item) => item !== code);
	}

	async function publishChangeSet() {
		if (!canManage || !preview || blockingFindings.length > 0 || !warningsAcknowledged) return;
		publishing = true;
		errorMessage = '';
		try {
			const updated = await publishAcademicTermChangeSet(changeSet.id, {
				rowVersion: preview.changeSetRowVersion,
				targetTimetableVersionRowVersion: preview.targetTimetableVersionRowVersion,
				previewHash: preview.previewHash,
				acknowledgedWarningCodes: [...new Set(warningFindings.map((finding) => finding.code))],
				idempotencyKey: crypto.randomUUID()
			});
			await onChanged(updated);
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict(
					'ข้อมูลเปลี่ยนหลังตรวจความพร้อม กรุณาตรวจความพร้อมใหม่ก่อนเผยแพร่'
				);
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่การเปลี่ยนแปลงไม่สำเร็จ';
		} finally {
			publishing = false;
		}
	}

	async function cancelDraft() {
		if (!canManage) return;
		cancelling = true;
		errorMessage = '';
		try {
			const updated = await cancelAcademicTermChangeSet(changeSet.id, {
				rowVersion: changeSet.rowVersion
			});
			await onChanged(updated);
			preview = null;
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict('แบบร่างถูกแก้ไขจากที่อื่น กรุณาตรวจรายการล่าสุดแล้วลองใหม่');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'ยกเลิกแบบร่างไม่สำเร็จ';
		} finally {
			cancelling = false;
		}
	}

	function findingRoute(finding: AcademicChangeFinding): string | null {
		return finding.route ?? null;
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
						<h2 class="font-semibold">การเปลี่ยนแปลงกลางภาค</h2>
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
				<p class="text-xs text-muted-foreground">เริ่มมีผล</p>
				<p class="font-medium text-amber-900">{formatDate(changeSet.effectiveFrom)}</p>
			</div>
		</div>
		<div class="mt-4 flex items-center gap-2 text-xs text-muted-foreground">
			<span class="size-2 rounded-full bg-muted-foreground/40"></span><span>ข้อมูลเดิม</span>
			<div class="h-px flex-1 bg-amber-500/30"></div>
			<span class="rounded-full bg-amber-500/15 px-2 py-1 font-medium text-amber-900">
				ตั้งแต่ {formatDate(changeSet.effectiveFrom)}
			</span>
			<div class="h-px flex-1 bg-primary/25"></div>
			<span class="size-2 rounded-full bg-primary"></span><span>ชุดใหม่</span>
		</div>
	</header>

	<div class="grid min-w-0 gap-5 p-4 sm:p-5 xl:grid-cols-[minmax(0,1fr)_320px]">
		<div class="min-w-0 space-y-5">
			<section class="space-y-3">
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div>
						<h3 class="font-medium">รายการที่จะเปลี่ยน</h3>
						<p class="text-xs text-muted-foreground">
							หลักสูตรไม่เปลี่ยน รายการเหล่านี้มีผลเฉพาะภาคเรียนนี้
						</p>
					</div>
					{#if canManage && changeSet.status === 'draft'}
						<Button size="sm" variant="outline" onclick={showItemForm}>
							<Plus class="size-4" /> เพิ่มรายการ
						</Button>
					{/if}
				</div>

				{#if changeSet.items.length === 0}
					<div
						class="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground"
					>
						ยังไม่มีรายการเพิ่ม ปรับคาบ หรือหยุดสอน
					</div>
				{:else}
					<div class="divide-y rounded-xl border">
						{#each changeSet.items as item (item.id)}
							<div class="flex items-center justify-between gap-3 p-3">
								<div class="min-w-0">
									<p class="font-medium">
										{item.actionKind === 'add_offering'
											? 'เพิ่มรายการเปิดสอน'
											: item.actionKind === 'stop_offering'
												? 'หยุดรายการเปิดสอน'
												: 'ปรับจำนวนคาบต่อสัปดาห์'}
									</p>
									<p class="truncate text-sm text-muted-foreground">
										{offerings.find((entry) => entry.offering.id === item.learningOfferingId)
											?.offering.nameSnapshot ?? item.learningOfferingId}
										{#if 'weeklyPeriodTarget' in item}
											· {item.weeklyPeriodTarget} คาบ/สัปดาห์
										{/if}
									</p>
								</div>
								<div class="flex shrink-0 gap-1">
									{#if item.actionKind === 'add_offering'}
										<Button
											href={`/staff/academic/delivery/${item.learningOfferingId}?timetableVersionId=${changeSet.targetTimetableVersionId}`}
											size="sm"
											variant="ghost"
										>
											{changeSet.status === 'draft' ? 'จัดกลุ่มและครู' : 'ดูรายละเอียด'}
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
								<div class="space-y-2 sm:col-span-2">
									<Label>หน่วยงานเจ้าของรายการ</Label>
									<DeliveryOptionCombobox
										bind:value={owningOrganizationUnitId}
										options={managementOptions.organizationUnits.map((unit) => ({
											id: unit.id,
											label: unit.name,
											description: unit.code
										}))}
										placeholder="เลือกหน่วยงานเจ้าของ"
									/>
								</div>
							</div>
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
										{selectedOffering?.snapshot.kind === 'course'
											? selectedOffering.snapshot.standardPeriodsPerWeek
											: 'ไม่มีค่ามาตรฐาน'}
										{selectedOffering?.snapshot.kind === 'course' ? 'คาบ/สัปดาห์' : ''}
									</p>
								</div>
								<div class="space-y-1">
									<Label for="adjust-weekly-period-target">จัดจริงภาคเรียนนี้</Label>
									<Input
										id="adjust-weekly-period-target"
										type="number"
										min="1"
										bind:value={weeklyPeriodTarget}
									/>
								</div>
							</div>
						{:else if selectedOffering}
							<p
								class="rounded-lg border border-rose-500/25 bg-rose-500/5 px-3 py-2 text-sm text-rose-800"
							>
								หยุดสอนตั้งแต่ {formatDate(changeSet.effectiveFrom)} โดยข้อมูลคะแนน ผลการเรียน และประวัติเดิมยังคงอยู่
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

			{#if changeSet.status === 'draft'}
				<section class="space-y-3">
					<div class="flex flex-wrap items-center justify-between gap-3">
						<div>
							<h3 class="font-medium">ตรวจผลกระทบและความพร้อม</h3>
							<p class="text-xs text-muted-foreground">ตรวจจากข้อมูลล่าสุดทุกครั้งก่อนเผยแพร่</p>
						</div>
						<Button variant="outline" size="sm" onclick={refreshPreview} disabled={loadingPreview}>
							<RefreshCw class={loadingPreview ? 'size-4 animate-spin' : 'size-4'} /> ตรวจความพร้อม
						</Button>
					</div>

					{#if preview}
						<div class="grid gap-3 sm:grid-cols-2">
							<div class="rounded-xl border border-destructive/25 bg-destructive/5 p-3">
								<p class="flex items-center gap-2 font-medium text-destructive">
									<CircleAlert class="size-4" /> จุดที่ต้องแก้ {blockingFindings.length}
								</p>
								<div class="mt-2 space-y-2">
									{#each blockingFindings as finding (`${finding.code}:${finding.resourceId ?? ''}:${finding.learningGroupId ?? ''}`)}
										<div class="rounded-lg bg-background/80 p-2 text-sm">
											<p class="font-medium">{finding.title}</p>
											<p class="text-xs text-muted-foreground">{finding.guidance}</p>
											{#if findingRoute(finding)}<Button
													href={findingRoute(finding) ?? undefined}
													size="sm"
													variant="link"
													class="h-auto px-0 py-1">ไปแก้ไข <ExternalLink class="size-3" /></Button
												>{/if}
										</div>
									{:else}
										<p class="text-sm text-emerald-700">ไม่มีจุดบล็อกการเผยแพร่</p>
									{/each}
								</div>
							</div>
							<div class="rounded-xl border border-amber-500/25 bg-amber-500/5 p-3">
								<p class="flex items-center gap-2 font-medium text-amber-900">
									<TriangleAlert class="size-4" /> คำเตือน {warningFindings.length}
								</p>
								<div class="mt-2 space-y-2">
									{#each warningFindings as finding (`${finding.code}:${finding.resourceId ?? ''}`)}
										<label
											class="flex cursor-pointer items-start gap-2 rounded-lg bg-background/80 p-2 text-sm"
										>
											<Checkbox
												checked={acknowledgedWarnings.includes(finding.code)}
												onCheckedChange={(checked) =>
													setWarningAcknowledgement(finding.code, checked)}
												aria-label={`รับทราบ ${finding.title}`}
											/>
											<span
												><span class="font-medium">{finding.title}</span><span
													class="block text-xs text-muted-foreground">{finding.guidance}</span
												>{#if finding.code === 'weekly_period_excess'}<span
														class="mt-1 block text-xs font-medium text-amber-800"
														>รับทราบว่าคาบจริงมากกว่าเป้าหมาย (weekly_period_excess)</span
													>{/if}</span
											>
										</label>
									{:else}
										<p class="text-sm text-muted-foreground">ไม่มีคำเตือนที่ต้องรับทราบ</p>
									{/each}
								</div>
							</div>
						</div>

						{#if preview.scheduleCounts.length > 0}
							<div class="overflow-x-auto rounded-xl border">
								<Table.Root>
									<Table.Header
										><Table.Row
											><Table.Head>กลุ่มเรียน</Table.Head><Table.Head class="text-end"
												>จัดแล้ว</Table.Head
											><Table.Head class="text-end">เป้าหมาย</Table.Head></Table.Row
										></Table.Header
									>
									<Table.Body>
										{#each preview.scheduleCounts as count (`${count.learningOfferingId}:${count.learningGroupId}`)}
											<Table.Row
												><Table.Cell>{count.learningGroupLabel}</Table.Cell><Table.Cell
													class="text-end font-mono">{count.actualPeriods}</Table.Cell
												><Table.Cell class="text-end font-mono">{count.targetPeriods}</Table.Cell
												></Table.Row
											>
										{/each}
									</Table.Body>
								</Table.Root>
							</div>
						{/if}
					{:else if loadingPreview}
						<div class="h-32 animate-pulse rounded-xl bg-muted"></div>
					{:else}
						<PageState
							variant="empty"
							title="ยังไม่ได้ตรวจความพร้อม"
							description="บันทึกรายการที่ต้องการ แล้วตรวจผลกระทบก่อนจัดตารางและเผยแพร่"
						/>
					{/if}
				</section>
			{/if}
		</div>

		<aside class="space-y-4 xl:sticky xl:top-4 xl:self-start">
			{#if changeSet.status === 'draft'}
				<div class="rounded-xl border bg-muted/20 p-4">
					<h3 class="font-medium">รุ่นตารางสอนหลังเปลี่ยน</h3>
					<p class="mt-1 text-xs text-muted-foreground">
						จัดตารางในรุ่นแบบร่างนี้เท่านั้น รุ่นเดิมยังไม่ถูกแก้ไข
					</p>
					<Button
						href={`/staff/academic/timetable?timetableVersionId=${changeSet.targetTimetableVersionId}`}
						variant="outline"
						class="mt-3 w-full"
					>
						เปิดรุ่นตารางแบบร่าง <ExternalLink class="size-4" />
					</Button>
				</div>
			{:else if changeSet.status === 'published'}
				<div class="rounded-xl border border-emerald-500/25 bg-emerald-500/5 p-4">
					<h3 class="font-medium text-emerald-900">ชุดนี้เผยแพร่แล้ว</h3>
					<p class="mt-1 text-xs text-muted-foreground">
						มีผลตั้งแต่ {formatDate(changeSet.effectiveFrom)}{changeSet.publishedAt
							? ` · เผยแพร่ ${formatDateTime(changeSet.publishedAt)}`
							: ''}
					</p>
					<Button
						href={`/staff/academic/timetable?timetableVersionId=${changeSet.targetTimetableVersionId}`}
						variant="outline"
						class="mt-3 w-full"
					>
						เปิดรุ่นตารางที่เผยแพร่ <ExternalLink class="size-4" />
					</Button>
				</div>
			{:else}
				<div class="rounded-xl border bg-muted/20 p-4">
					<h3 class="font-medium">แบบร่างนี้ยกเลิกแล้ว</h3>
					<p class="mt-1 text-xs text-muted-foreground">
						ไม่มีผลต่อรายการเปิดสอน กลุ่มเรียน ครู และตารางสอน
					</p>
				</div>
			{/if}

			{#if preview}
				<div class="rounded-xl border p-4">
					<h3 class="font-medium">ข้อมูลที่เกี่ยวข้อง</h3>
					<p class="mt-1 text-xs text-muted-foreground">
						จำนวนอ้างอิงเพื่อประเมินผลกระทบ ไม่ได้ลบข้อมูลเดิม
					</p>
					<dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-2 text-sm">
						{#each impactItems as [label, value] (label)}
							<div class="rounded-lg bg-muted/40 px-2.5 py-2">
								<dt class="text-[11px] text-muted-foreground">{label}</dt>
								<dd class="font-mono font-semibold tabular-nums">{value}</dd>
							</div>
						{/each}
					</dl>
					<p class="mt-3 text-xs leading-relaxed text-muted-foreground">
						ข้อมูลเดิมยังคงอยู่ รวมถึงโครงสร้างคะแนน ผลการเรียน ตารางสอบ และประวัตินิเทศ
					</p>
				</div>
			{/if}

			{#if canManage && changeSet.status === 'draft'}
				<div class="space-y-2 rounded-xl border border-primary/20 bg-primary/[0.025] p-4">
					<div class="flex items-center gap-2">
						{#if preview && blockingFindings.length === 0 && warningsAcknowledged}<CheckCircle2
								class="size-4 text-emerald-600"
							/>{:else}<Clock3 class="size-4 text-muted-foreground" />{/if}
						<h3 class="font-medium">เผยแพร่ชุดใหม่</h3>
					</div>
					<p class="text-xs text-muted-foreground">
						ต้องไม่มีจุดบล็อก และรับทราบคำเตือนปัจจุบันทุกข้อ
					</p>
					<LoadingButton
						class="w-full"
						loading={publishing}
						loadingLabel="กำลังเผยแพร่"
						disabled={!preview || blockingFindings.length > 0 || !warningsAcknowledged}
						onclick={publishChangeSet}
					>
						<Send class="size-4" /> เผยแพร่ตั้งแต่ {formatDate(changeSet.effectiveFrom)}
					</LoadingButton>
					<LoadingButton
						class="w-full"
						variant="ghost"
						loading={cancelling}
						loadingLabel="กำลังยกเลิก"
						onclick={cancelDraft}
					>
						ยกเลิกแบบร่าง
					</LoadingButton>
				</div>
			{/if}

			{#if errorMessage && !itemFormOpen}
				<p
					role="alert"
					class="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-sm text-destructive"
				>
					{errorMessage}
				</p>
			{/if}
		</aside>
	</div>
</section>
