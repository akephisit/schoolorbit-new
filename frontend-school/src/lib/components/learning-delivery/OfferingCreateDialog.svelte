<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { DeliveryOfferingAddKind } from '#lib/academic/learning-delivery-page.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		createLearningOffering,
		getLearningDeliveryManagementOptions,
		type DeliveryManagementOptions,
		type LearningOfferingOverviewItem
	} from '#lib/api/learning-delivery.js';
	import type { CurriculumPreparationTarget } from '#lib/academic/synchronized-activity-delivery.js';
	import { goto } from '$app/navigation';
	import { deliveryOfferingHref } from '#lib/navigation/learning-delivery.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { BookCopy, Plus, Sparkles } from '@lucide/svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';
	import OfferingCurriculumPreview from './OfferingCurriculumPreview.svelte';

	let {
		academicTermId,
		onCreated,
		onApplied,
		defaultDeliveryVersionId = null,
		returnTo = '',
		showTrigger = true
	}: {
		academicTermId: string;
		onCreated: (item: LearningOfferingOverviewItem) => void;
		onApplied: () => Promise<void>;
		defaultDeliveryVersionId?: string | null;
		returnTo?: string;
		showTrigger?: boolean;
	} = $props();

	let open = $state(false);
	let returnFocus = $state<HTMLElement | null>(null);
	let mode = $state<'curriculum' | 'manual'>('curriculum');
	let addKind = $state<DeliveryOfferingAddKind | null>(null);
	const optionsRequest = new LatestRequest();
	onDestroy(() => optionsRequest.abort());
	let preparationTarget = $state.raw<CurriculumPreparationTarget | null>(null);
	let deliveryVersionId = $state<string | null>(null);
	let preparationRevision = $state(0);
	let options = $state.raw<DeliveryManagementOptions | null>(null);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let startsOn = $state('');
	let activityOpeningMode = $state<'central' | 'groups'>('central');
	let errorMessage = $state('');
	let draft = $state({
		kind: 'course' as 'course' | 'activity',
		catalogVersionId: '',
		gradeLevelId: '',
		studyProgramId: ''
	});
	let catalogOptions = $derived(
		(options?.catalogVersions ?? [])
			.filter(
				(item) =>
					item.kind === draft.kind &&
					(!addKind ||
						addKind === 'activity' ||
						item.subjectType === (addKind === 'basic' ? 'BASIC' : 'ADDITIONAL'))
			)
			.sort((left, right) =>
				(left.kind === 'course' ? left.code : left.name).localeCompare(
					right.kind === 'course' ? right.code : right.name,
					'th-TH',
					{ numeric: true }
				)
			)
			.map((item) => ({
				id: item.id,
				label: item.kind === 'activity' ? `${item.name} (ฉบับ ${item.versionNo})` : item.label,
				description:
					item.kind === 'course' && item.standardPeriodsPerWeek != null
						? `มาตรฐาน ${item.standardPeriodsPerWeek} คาบ/สัปดาห์`
						: item.kind === 'course'
							? 'รายวิชา'
							: 'กิจกรรม'
			}))
	);

	async function showDialog(
		target: CurriculumPreparationTarget | null,
		targetDeliveryVersionId: string | null = defaultDeliveryVersionId,
		requestedKind: DeliveryOfferingAddKind | null = null
	) {
		if (!open && document.activeElement instanceof HTMLElement)
			returnFocus = document.activeElement;
		preparationTarget = target;
		deliveryVersionId = targetDeliveryVersionId;
		preparationRevision += 1;
		addKind = requestedKind;
		mode = requestedKind ? 'manual' : 'curriculum';
		if (requestedKind) {
			startsOn = '';
			draft = {
				...draft,
				kind: requestedKind === 'activity' ? 'activity' : 'course',
				catalogVersionId: ''
			};
		}
		open = true;
		if (options || optionsLoading) return;
		optionsLoading = true;
		errorMessage = '';
		const ticket = optionsRequest.begin();
		try {
			const loaded = await getLearningDeliveryManagementOptions(academicTermId, {
				signal: ticket.signal
			});
			if (optionsRequest.isCurrent(ticket.revision)) options = loaded;
		} catch (error) {
			if (isAbortError(error) || !optionsRequest.isCurrent(ticket.revision)) return;
			errorMessage =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับเปิดการเรียนไม่สำเร็จ';
		} finally {
			optionsLoading = false;
		}
	}

	export function openCurriculumPreparation(
		target: CurriculumPreparationTarget,
		targetDeliveryVersionId: string | null = defaultDeliveryVersionId
	) {
		return showDialog(target, targetDeliveryVersionId);
	}

	export function openManual(kind: DeliveryOfferingAddKind) {
		return showDialog(null, defaultDeliveryVersionId, kind);
	}

	function selectMode(nextMode: 'curriculum' | 'manual') {
		mode = nextMode;
		errorMessage = '';
	}

	async function createManual(event: SubmitEvent) {
		event.preventDefault();
		if (
			!options ||
			!startsOn ||
			!draft.catalogVersionId ||
			!draft.gradeLevelId ||
			!draft.studyProgramId
		)
			return;
		saving = true;
		errorMessage = '';
		try {
			const catalog = options.catalogVersions.find(
				(item) => item.id === draft.catalogVersionId && item.kind === draft.kind
			);
			if (!catalog || (draft.kind === 'activity' && !catalog.schedulingMode))
				throw new Error('ไม่พบรูปแบบกิจกรรม กรุณาโหลดตัวเลือกอีกครั้ง');
			const targets = [
				{
					targetKind: 'grade_program' as const,
					homeroomId: null,
					gradeLevelId: draft.gradeLevelId,
					studyProgramId: draft.studyProgramId
				}
			];
			const offering = await createLearningOffering(
				draft.kind === 'course'
					? {
							kind: 'course',
							academicTermId,
							startsOn,
							subjectVersionId: draft.catalogVersionId,
							curriculumCourseRequirementId: null,
							assessmentTotalScore: '100.00',
							targets
						}
					: {
							kind: 'activity',
							academicTermId,
							startsOn,
							activityVersionId: draft.catalogVersionId,
							curriculumActivityRequirementId: null,
							registrationType: 'assigned',
							schedulingMode: catalog.schedulingMode!,
							createHomeroomGroups: activityOpeningMode === 'groups',
							capacity: null,
							attendanceRequirement: { minimumPercent: '80.00', requiredSessions: null },
							passCriteria: {
								requireAttendance: true,
								requireTeacherConfirmation: true,
								outcomes: ['pass', 'fail']
							},
							targets
						},
				deliveryVersionId
			);
			onCreated({
				offering,
				gradeLevels: options.gradeLevels.filter((grade) => grade.id === draft.gradeLevelId),
				studyPrograms: options.studyPrograms.filter(
					(program) => program.id === draft.studyProgramId
				),
				groupCount: 0,
				teacherAssignmentCount: 0,
				groupsWithoutPrimaryTeacher: 0,
				publishedRosterCount: 0
			});
			draft = { ...draft, catalogVersionId: '' };
			open = false;
			if (draft.kind === 'activity' && activityOpeningMode === 'groups')
				await goto(deliveryOfferingHref(offering.id, { ...offering, deliveryVersionId }, returnTo));
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างรายการเปิดสอนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	async function applied() {
		await onApplied();
		open = false;
	}
</script>

{#if showTrigger}<Button onclick={() => showDialog(null)}
		><Plus class="size-4" /> เปิดการเรียนการสอน</Button
	>{/if}

<Dialog.Root bind:open>
	<Dialog.Content
		class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-3xl"
		showCloseButton={!saving}
		escapeKeydownBehavior={saving ? 'ignore' : 'close'}
		interactOutsideBehavior={saving ? 'ignore' : 'close'}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (returnFocus?.isConnected) returnFocus.focus();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>
				{addKind
					? addKind === 'basic'
						? 'เพิ่มวิชาพื้นฐาน'
						: addKind === 'additional'
							? 'เพิ่มวิชาเพิ่มเติม'
							: 'เพิ่มกิจกรรมพัฒนาผู้เรียน'
					: preparationTarget
						? preparationTarget.resourceKind === 'activity'
							? `เพิ่มกิจกรรม ${preparationTarget.name}`
							: `เปิดรายวิชา ${preparationTarget.name}`
						: 'เปิดการเรียนการสอนในภาคเรียนนี้'}
			</Dialog.Title>
			<Dialog.Description
				>{addKind
					? addKind === 'basic'
						? 'เพิ่มวิชาพื้นฐาน'
						: addKind === 'additional'
							? 'เพิ่มวิชาเพิ่มเติม'
							: 'เพิ่มกิจกรรมพัฒนาผู้เรียน'
					: preparationTarget
						? preparationTarget.schedulingMode === 'synchronized'
							? `นำกิจกรรมจากหลักสูตรมาเปิดเป็นรายการกลางสำหรับ ${preparationTarget.homeroomCount} ห้อง`
							: `เตรียมรายการตามหลักสูตรและตรวจกลุ่มสำหรับ ${preparationTarget.homeroomCount} ห้อง`
						: 'เลือกนำรายการจากหลักสูตร หรือเพิ่มรายการเฉพาะภาคเรียนเอง'}</Dialog.Description
			>
		</Dialog.Header>
		{#if preparationTarget}<p class="text-sm text-muted-foreground">
				การเพิ่มนี้ครอบคลุมทุกห้องที่ยังขาดฉบับนี้ รวม {preparationTarget.homeroomCount} ห้อง ระบบจะตรวจข้อจำกัดก่อนบันทึก
			</p>{/if}

		{#if optionsLoading}
			<div class="space-y-3 py-4" aria-label="กำลังโหลดตัวเลือก">
				<div class="h-16 animate-pulse rounded-xl bg-muted"></div>
				<div class="h-44 animate-pulse rounded-xl bg-muted"></div>
			</div>
		{:else if options}
			{#if !preparationTarget && !addKind}<div
					class="grid grid-cols-2 gap-2 rounded-xl bg-muted/60 p-1.5"
					aria-label="วิธีเพิ่มรายการเปิดสอน"
				>
					<Button
						type="button"
						variant={mode === 'curriculum' ? 'default' : 'ghost'}
						class="h-auto justify-start px-3 py-2.5"
						onclick={() => selectMode('curriculum')}
						><BookCopy class="size-4" /><span class="text-start"
							><span class="block">นำมาจากหลักสูตร</span><span
								class="block text-[11px] font-normal opacity-75">เหมาะกับการเปิดภาคเรียนตามแผน</span
							></span
						></Button
					>
					<Button
						type="button"
						variant={mode === 'manual' ? 'default' : 'ghost'}
						class="h-auto justify-start px-3 py-2.5"
						onclick={() => selectMode('manual')}
						><Sparkles class="size-4" /><span class="text-start"
							><span class="block">เพิ่มรายการเอง</span><span
								class="block text-[11px] font-normal opacity-75"
								>สำหรับวิชาหรือกิจกรรมเฉพาะครั้ง</span
							></span
						></Button
					>
				</div>{/if}

			{#if mode === 'curriculum'}
				{#key preparationRevision}
					<OfferingCurriculumPreview
						{academicTermId}
						{options}
						{preparationTarget}
						{deliveryVersionId}
						onApplied={applied}
					/>
				{/key}
			{:else}
				<form class="space-y-4" onsubmit={createManual}>
					<fieldset disabled={saving} class="space-y-4">
						<div class="space-y-2">
							<Label for="offering-actual-start">วันที่เริ่มเปิดสอนจริง</Label><DatePicker
								id="offering-actual-start"
								bind:value={startsOn}
								ariaLabel="วันที่เริ่มเปิดสอนจริง"
								required
							/>
						</div>
						<div class="grid gap-4 sm:grid-cols-2">
							<div class="space-y-2">
								<Label for="offering-create-kind">ประเภท</Label>{#if addKind}<p
										class="py-2 text-sm"
									>
										{addKind === 'basic'
											? 'วิชาพื้นฐาน'
											: addKind === 'additional'
												? 'วิชาเพิ่มเติม'
												: 'กิจกรรมพัฒนาผู้เรียน'}
									</p>{:else}<Select.Root
										type="single"
										bind:value={draft.kind}
										onValueChange={() => (draft.catalogVersionId = '')}
										><Select.Trigger id="offering-create-kind" class="w-full"
											>{draft.kind === 'course'
												? 'รายวิชา'
												: 'กิจกรรมพัฒนาผู้เรียน'}</Select.Trigger
										><Select.Content
											><Select.Item value="course">รายวิชา</Select.Item><Select.Item
												value="activity">กิจกรรมพัฒนาผู้เรียน</Select.Item
											></Select.Content
										></Select.Root
									>{/if}
							</div>
							<div class="space-y-2">
								<Label>รายวิชาหรือกิจกรรม</Label><DeliveryOptionCombobox
									bind:value={draft.catalogVersionId}
									options={catalogOptions}
									placeholder={draft.kind === 'course' ? 'เลือกรายวิชา' : 'เลือกกิจกรรม'}
									searchPlaceholder="ค้นหารหัสหรือชื่อ..."
								/>
							</div>
							<div class="space-y-2">
								<Label>ระดับชั้น</Label><DeliveryOptionCombobox
									bind:value={draft.gradeLevelId}
									options={options.gradeLevels.map((grade) => ({
										id: grade.id,
										label: grade.name,
										description: grade.short_name ?? grade.code
									}))}
									placeholder="เลือกระดับชั้น"
								/>
							</div>
							<div class="space-y-2">
								<Label>แผนการเรียน</Label><DeliveryOptionCombobox
									bind:value={draft.studyProgramId}
									options={options.studyPrograms.map((program) => ({
										id: program.id,
										label: program.name,
										description: `${program.levelName} · ${program.code}`
									}))}
									placeholder="เลือกแผนการเรียน"
								/>
							</div>
						</div>
						{#if draft.kind === 'activity' && options.catalogVersions.find((item) => item.id === draft.catalogVersionId)?.schedulingMode === 'synchronized'}
							<div class="space-y-2">
								<Label for="activity-opening-mode">รูปแบบการเปิด</Label><Select.Root
									type="single"
									bind:value={activityOpeningMode}
									><Select.Trigger id="activity-opening-mode" class="w-full"
										>{activityOpeningMode === 'central'
											? 'เปิดแบบกลาง'
											: 'เปิดพร้อมจัดกลุ่ม'}</Select.Trigger
									><Select.Content
										><Select.Item value="central">เปิดแบบกลาง</Select.Item><Select.Item
											value="groups">เปิดพร้อมจัดกลุ่ม</Select.Item
										></Select.Content
									></Select.Root
								>
								<p class="text-xs text-muted-foreground">
									เปิดแบบกลางจัดคาบร่วมกันได้ทันที เปิดพร้อมจัดกลุ่มสร้างหนึ่งกลุ่มต่อห้องเป้าหมาย
									แล้วมอบหมายครูก่อนเผยแพร่
								</p>
							</div>
						{/if}
						{#if draft.kind === 'course' && draft.catalogVersionId}{@const selected =
								options.catalogVersions.find((item) => item.id === draft.catalogVersionId)}
							<p class="text-sm text-muted-foreground">
								ตามหลักสูตร {selected?.standardPeriodsPerWeek ?? '—'} คาบ/สัปดาห์ · ปรับจำนวนคาบได้ในรายละเอียดหลังเพิ่มรายการ
							</p>{/if}
						<p
							class="rounded-lg bg-muted/45 px-3 py-2 text-xs leading-relaxed text-muted-foreground"
						>
							ระบบจะใช้กลุ่มสาระหรือกิจกรรมพัฒนาผู้เรียนจากทะเบียนโดยอัตโนมัติ และสร้างเป็นฉบับร่าง
							กิจกรรมกลางจัดกลุ่มและกำหนดครูภายหลังได้
							ส่วนรายวิชาและกลุ่มที่สร้างแล้วต้องตรวจกลุ่มและครูก่อนเผยแพร่
						</p>
						{#if errorMessage}<p role="alert" class="text-sm text-destructive">
								{errorMessage}
							</p>{/if}
					</fieldset>
					<Dialog.Footer
						><Button
							type="button"
							variant="outline"
							disabled={saving}
							onclick={() => (open = false)}>ยกเลิก</Button
						><LoadingButton
							type="submit"
							loading={saving}
							loadingLabel="กำลังสร้าง"
							disabled={!startsOn ||
								!draft.catalogVersionId ||
								!draft.gradeLevelId ||
								!draft.studyProgramId}>สร้างฉบับร่าง</LoadingButton
						></Dialog.Footer
					>
				</form>
			{/if}
		{:else}
			<div class="space-y-3 py-5">
				<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
				<Button
					type="button"
					variant="outline"
					onclick={() => showDialog(preparationTarget, deliveryVersionId, addKind)}
					>ลองโหลดอีกครั้ง</Button
				>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
