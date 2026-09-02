<script lang="ts">
	import {
		createLearningOffering,
		getLearningDeliveryManagementOptions,
		type DeliveryManagementOptions,
		type LearningOfferingOverviewItem
	} from '$lib/api/learning-delivery';
	import type { SynchronizedActivityPreparationTarget } from '$lib/academic/synchronized-activity-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookCopy, Plus, Sparkles } from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';
	import OfferingCurriculumPreview from './OfferingCurriculumPreview.svelte';

	let {
		academicTermId,
		onCreated,
		onApplied
	}: {
		academicTermId: string;
		onCreated: (item: LearningOfferingOverviewItem) => void;
		onApplied: () => Promise<void>;
	} = $props();

	let open = $state(false);
	let mode = $state<'curriculum' | 'manual'>('curriculum');
	let preparationTarget = $state.raw<SynchronizedActivityPreparationTarget | null>(null);
	let preparationRevision = $state(0);
	let options = $state.raw<DeliveryManagementOptions | null>(null);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let draft = $state({
		kind: 'course' as 'course' | 'activity',
		catalogVersionId: '',
		owningOrganizationUnitId: '',
		gradeLevelId: '',
		studyProgramId: ''
	});
	let catalogOptions = $derived(
		(options?.catalogVersions ?? [])
			.filter((item) => item.kind === draft.kind)
			.map((item) => ({
				id: item.id,
				label: item.label,
				description: item.kind === 'course' ? 'รายวิชา' : 'กิจกรรม'
			}))
	);

	async function showDialog(target: SynchronizedActivityPreparationTarget | null) {
		preparationTarget = target;
		preparationRevision += 1;
		mode = 'curriculum';
		open = true;
		if (options || optionsLoading) return;
		optionsLoading = true;
		errorMessage = '';
		try {
			options = await getLearningDeliveryManagementOptions(academicTermId);
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับเปิดการเรียนไม่สำเร็จ';
		} finally {
			optionsLoading = false;
		}
	}

	export function openCurriculumPreparation(target: SynchronizedActivityPreparationTarget) {
		return showDialog(target);
	}

	function selectMode(nextMode: 'curriculum' | 'manual') {
		mode = nextMode;
		errorMessage = '';
	}

	async function createManual(event: SubmitEvent) {
		event.preventDefault();
		if (
			!options ||
			!draft.catalogVersionId ||
			!draft.owningOrganizationUnitId ||
			!draft.gradeLevelId ||
			!draft.studyProgramId
		)
			return;
		saving = true;
		errorMessage = '';
		try {
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
							subjectVersionId: draft.catalogVersionId,
							curriculumCourseRequirementId: null,
							owningOrganizationUnitId: draft.owningOrganizationUnitId,
							gradingPolicy: {
								policyCode: 'school_default',
								totalScore: '100.00',
								passingScore: '50.00'
							},
							targets
						}
					: {
							kind: 'activity',
							academicTermId,
							activityVersionId: draft.catalogVersionId,
							curriculumActivityRequirementId: null,
							owningOrganizationUnitId: draft.owningOrganizationUnitId,
							registrationType: 'assigned',
							schedulingMode: 'synchronized',
							capacity: null,
							attendanceRequirement: { minimumPercent: '80.00', requiredSessions: null },
							passCriteria: {
								requireAttendance: true,
								requireTeacherConfirmation: true,
								outcomes: ['pass', 'fail']
							},
							targets
						}
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

<Button onclick={() => showDialog(null)}><Plus class="size-4" /> เปิดการเรียนการสอน</Button>

<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
		<Dialog.Header>
			<Dialog.Title>
				{preparationTarget
					? `เปิดใช้งาน ${preparationTarget.code} · ${preparationTarget.name}`
					: 'เปิดการเรียนการสอนในภาคเรียนนี้'}
			</Dialog.Title>
			<Dialog.Description
				>{preparationTarget
					? `นำกิจกรรมจากหลักสูตรมาเปิดเป็นรายการกลางสำหรับ ${preparationTarget.homeroomCount} ห้อง`
					: 'เลือกนำรายการจากหลักสูตร หรือเพิ่มรายการเฉพาะภาคเรียนเอง'}</Dialog.Description
			>
		</Dialog.Header>

		{#if optionsLoading}
			<div class="space-y-3 py-4" aria-label="กำลังโหลดตัวเลือก">
				<div class="h-16 animate-pulse rounded-xl bg-muted"></div>
				<div class="h-44 animate-pulse rounded-xl bg-muted"></div>
			</div>
		{:else if options}
			{#if !preparationTarget}<div
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
						onApplied={applied}
					/>
				{/key}
			{:else}
				<form class="space-y-4" onsubmit={createManual}>
					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<Label>ประเภท</Label><Select.Root
								type="single"
								bind:value={draft.kind}
								onValueChange={() => (draft.catalogVersionId = '')}
								><Select.Trigger class="w-full"
									>{draft.kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน'}</Select.Trigger
								><Select.Content
									><Select.Item value="course">รายวิชา</Select.Item><Select.Item value="activity"
										>กิจกรรมพัฒนาผู้เรียน</Select.Item
									></Select.Content
								></Select.Root
							>
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
									description: `${program.curriculumName} · ${program.code}`
								}))}
								placeholder="เลือกแผนการเรียน"
							/>
						</div>
						<div class="space-y-2 sm:col-span-2">
							<Label>หน่วยงานเจ้าของรายการ</Label><DeliveryOptionCombobox
								bind:value={draft.owningOrganizationUnitId}
								options={options.organizationUnits.map((unit) => ({
									id: unit.id,
									label: unit.name,
									description: unit.code
								}))}
								placeholder="เลือกหน่วยงานเจ้าของ"
							/>
						</div>
					</div>
					<p class="rounded-lg bg-muted/45 px-3 py-2 text-xs leading-relaxed text-muted-foreground">
						ระบบจะสร้างเป็นฉบับร่าง คุณยังต้องเพิ่มกลุ่มเรียน มอบหมายครู และตรวจรายชื่อก่อนเผยแพร่
					</p>
					{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
					<Dialog.Footer
						><Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button
						><LoadingButton
							type="submit"
							loading={saving}
							loadingLabel="กำลังสร้าง"
							disabled={!draft.catalogVersionId ||
								!draft.owningOrganizationUnitId ||
								!draft.gradeLevelId ||
								!draft.studyProgramId}>สร้างฉบับร่าง</LoadingButton
						></Dialog.Footer
					>
				</form>
			{/if}
		{:else}
			<div class="space-y-3 py-5">
				<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
				<Button type="button" variant="outline" onclick={() => showDialog(preparationTarget)}
					>ลองโหลดอีกครั้ง</Button
				>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
