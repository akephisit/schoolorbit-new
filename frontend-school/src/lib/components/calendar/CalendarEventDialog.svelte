<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { LoadingButton } from '$lib/components/app-state';
	import type {
		CalendarAudienceType,
		CalendarCategory,
		CalendarEvent,
		CalendarEventTargetInput,
		CreateCalendarEventRequest
	} from '$lib/api/calendar';
	import { cn } from '$lib/utils';

	type GradeLevelOption = { id: string; name: string };
	type ClassroomOption = { id: string; name: string; grade_level_id?: string };

	let {
		open = $bindable(false),
		event = null,
		categories = [],
		gradeLevels = [],
		classrooms = [],
		saving = false,
		onsave
	}: {
		open: boolean;
		event?: CalendarEvent | null;
		categories?: CalendarCategory[];
		gradeLevels?: GradeLevelOption[];
		classrooms?: ClassroomOption[];
		saving?: boolean;
		onsave?: (payload: CreateCalendarEventRequest) => void;
	} = $props();

	const audienceOptions: { value: CalendarAudienceType; label: string }[] = [
		{ value: 'all', label: 'ทุกคน' },
		{ value: 'staff', label: 'บุคลากร' },
		{ value: 'student', label: 'นักเรียน' },
		{ value: 'parent', label: 'ผู้ปกครอง' }
	];

	const fixedReminderDays = [1, 3, 7];

	let title = $state('');
	let description = $state('');
	let location = $state('');
	let categoryId = $state('');
	let startDate = $state('');
	let endDate = $state('');
	let allDay = $state(true);
	let startTime = $state('');
	let endTime = $state('');
	let isPublic = $state(false);
	let notifyAudience = $state(true);
	let selectedAudiences = $state<CalendarAudienceType[]>(['all']);
	let selectedGradeLevelId = $state('');
	let selectedClassRoomId = $state('');
	let reminder1Day = $state(true);
	let reminder3Days = $state(false);
	let reminder7Days = $state(false);
	let customReminderDays = $state('');
	let loadedEventId = $state<string | null | undefined>(undefined);

	let activeCategories = $derived(categories.filter((category) => category.isActive));
	let selectedCategoryLabel = $derived(
		activeCategories.find((category) => category.id === categoryId)?.name ?? 'ไม่ระบุหมวดหมู่'
	);
	let targetAudienceSelected = $derived(
		selectedAudiences.includes('student') || selectedAudiences.includes('parent')
	);
	let filteredClassrooms = $derived(
		selectedGradeLevelId
			? classrooms.filter((classroom) => classroom.grade_level_id === selectedGradeLevelId)
			: classrooms
	);
	let selectedGradeLevelLabel = $derived(
		gradeLevels.find((gradeLevel) => gradeLevel.id === selectedGradeLevelId)?.name ?? 'ทุกระดับชั้น'
	);
	let selectedClassroomLabel = $derived(
		filteredClassrooms.find((classroom) => classroom.id === selectedClassRoomId)?.name ??
			'ทุกห้องเรียน'
	);
	let hasMultipleTargetRows = $derived((event?.targets.length ?? 0) > 1);

	$effect(() => {
		if (!open) {
			loadedEventId = undefined;
			return;
		}

		const nextEventId = event?.id ?? null;
		if (loadedEventId !== nextEventId) {
			loadEvent(event);
			loadedEventId = nextEventId;
		}
	});

	$effect(() => {
		if (
			selectedGradeLevelId &&
			classrooms.length > 0 &&
			selectedClassRoomId &&
			!classrooms.some(
				(classroom) =>
					classroom.id === selectedClassRoomId && classroom.grade_level_id === selectedGradeLevelId
			)
		) {
			selectedClassRoomId = '';
		}
	});

	function loadEvent(source: CalendarEvent | null | undefined) {
		title = source?.title ?? '';
		description = source?.description ?? '';
		location = source?.location ?? '';
		categoryId = source?.categoryId ?? '';
		startDate = source?.startDate ?? '';
		endDate = source?.endDate ?? '';
		allDay = source?.allDay ?? true;
		startTime = source?.startTime?.slice(0, 5) ?? '';
		endTime = source?.endTime?.slice(0, 5) ?? '';
		isPublic = source?.isPublic ?? false;
		notifyAudience = source ? false : true;

		const audiences = source?.targets.map((target) => target.audienceType) ?? ['all'];
		selectedAudiences = uniqueAudiences(audiences.length > 0 ? audiences : ['all']);
		selectedGradeLevelId =
			source?.targets.find((target) => target.gradeLevelId)?.gradeLevelId ?? '';
		selectedClassRoomId = source?.targets.find((target) => target.classRoomId)?.classRoomId ?? '';

		const reminderDays = source?.reminders.map((reminder) => reminder.daysBefore) ?? [1];
		reminder1Day = reminderDays.includes(1);
		reminder3Days = reminderDays.includes(3);
		reminder7Days = reminderDays.includes(7);
		customReminderDays =
			reminderDays.find((daysBefore) => !fixedReminderDays.includes(daysBefore))?.toString() ?? '';
	}

	function toggleAudience(audienceType: CalendarAudienceType) {
		if (audienceType === 'all') {
			selectedAudiences = ['all'];
			selectedGradeLevelId = '';
			selectedClassRoomId = '';
			return;
		}

		const withoutAll = selectedAudiences.filter((value) => value !== 'all');
		if (withoutAll.includes(audienceType)) {
			const nextAudiences = withoutAll.filter((value) => value !== audienceType);
			selectedAudiences = nextAudiences.length > 0 ? nextAudiences : ['all'];
		} else {
			selectedAudiences = [...withoutAll, audienceType];
		}
	}

	function reminderOffsetsDays() {
		const days: number[] = [];
		addReminderDay(days, reminder1Day, 1);
		addReminderDay(days, reminder3Days, 3);
		addReminderDay(days, reminder7Days, 7);

		const customDays = Number(customReminderDays);
		if (Number.isInteger(customDays) && customDays > 0) {
			addReminderDay(days, true, customDays);
		}

		return days.sort((left, right) => left - right);
	}

	function addReminderDay(days: number[], enabled: boolean, value: number) {
		if (enabled && !days.includes(value)) {
			days.push(value);
		}
	}

	function uniqueAudiences(audiences: CalendarAudienceType[]) {
		const unique: CalendarAudienceType[] = [];
		for (const audienceType of audiences) {
			if (!unique.includes(audienceType)) {
				unique.push(audienceType);
			}
		}
		return unique;
	}

	function supportsTargetScope(audienceType: CalendarAudienceType) {
		return audienceType === 'student' || audienceType === 'parent';
	}

	function targetGradeLevelId(audienceType: CalendarAudienceType) {
		if (!supportsTargetScope(audienceType)) return null;
		return selectedClassRoomId ? null : selectedGradeLevelId || null;
	}

	function targetClassRoomId(audienceType: CalendarAudienceType) {
		if (!supportsTargetScope(audienceType)) return null;
		return selectedClassRoomId || null;
	}

	function submitForm() {
		if (hasMultipleTargetRows) return;

		const targets: CalendarEventTargetInput[] = selectedAudiences.map((audienceType) => ({
			audienceType,
			gradeLevelId: targetGradeLevelId(audienceType),
			classRoomId: targetClassRoomId(audienceType)
		}));

		onsave?.({
			title: title.trim(),
			description: description.trim() || null,
			location: location.trim() || null,
			categoryId: categoryId || null,
			startDate,
			endDate,
			allDay,
			startTime: allDay ? null : startTime || null,
			endTime: allDay ? null : endTime || null,
			isPublic,
			targets,
			reminderOffsetsDays: reminderOffsetsDays(),
			notifyAudience
		});
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[92vh] flex-col p-0 sm:max-w-3xl">
		<Dialog.Header class="border-b px-6 py-5">
			<Dialog.Title>{event ? 'แก้ไขกิจกรรมปฏิทิน' : 'สร้างกิจกรรมปฏิทิน'}</Dialog.Title>
			<Dialog.Description>กำหนดรายละเอียด วันเวลา ผู้ชม และการแจ้งเตือน</Dialog.Description>
		</Dialog.Header>

		<form
			class="min-h-0 flex-1 overflow-y-auto px-6 py-5"
			onsubmit={(submitEvent) => {
				submitEvent.preventDefault();
				submitForm();
			}}
		>
			<div class="space-y-6">
				{#if hasMultipleTargetRows}
					<div class="rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
						ไม่สามารถแก้ไขกลุ่มผู้ชมหลายรายการจากฟอร์มนี้ได้ เพื่อป้องกันการบันทึกทับกลุ่มเป้าหมายเดิม
					</div>
				{/if}

				<section class="grid gap-4">
					<div class="grid gap-2">
						<Label for="calendar-event-title">ชื่อกิจกรรม</Label>
						<Input id="calendar-event-title" bind:value={title} required maxlength={180} />
					</div>
					<div class="grid gap-2">
						<Label for="calendar-event-description">รายละเอียด</Label>
						<Textarea id="calendar-event-description" bind:value={description} class="min-h-24" />
					</div>
					<div class="grid gap-2">
						<Label for="calendar-event-location">สถานที่</Label>
						<Input id="calendar-event-location" bind:value={location} maxlength={180} />
					</div>
					<div class="grid gap-2">
						<Label>หมวดหมู่</Label>
						<Select.Root type="single" bind:value={categoryId}>
							<Select.Trigger class="w-full">{selectedCategoryLabel}</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ไม่ระบุหมวดหมู่</Select.Item>
								{#each activeCategories as category (category.id)}
									<Select.Item value={category.id}>{category.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</section>

				<section class="grid gap-4 md:grid-cols-2">
					<div class="grid gap-2">
						<Label>วันที่เริ่มต้น</Label>
						<DatePicker bind:value={startDate} />
					</div>
					<div class="grid gap-2">
						<Label>วันที่สิ้นสุด</Label>
						<DatePicker bind:value={endDate} />
					</div>
					<label class="flex items-center gap-2 text-sm font-medium md:col-span-2">
						<Checkbox bind:checked={allDay} />
						ทั้งวัน
					</label>
					{#if !allDay}
						<div class="grid gap-2">
							<Label for="calendar-event-start-time">เวลาเริ่มต้น</Label>
							<Input id="calendar-event-start-time" type="time" bind:value={startTime} />
						</div>
						<div class="grid gap-2">
							<Label for="calendar-event-end-time">เวลาสิ้นสุด</Label>
							<Input id="calendar-event-end-time" type="time" bind:value={endTime} />
						</div>
					{/if}
				</section>

				<section class="grid gap-3">
					<Label>ผู้ชม</Label>
					<div class="grid grid-cols-2 gap-2 md:grid-cols-4">
						{#each audienceOptions as option (option.value)}
							<Button
								type="button"
								variant={selectedAudiences.includes(option.value) ? 'default' : 'outline'}
								class="h-9"
								disabled={hasMultipleTargetRows}
								aria-pressed={selectedAudiences.includes(option.value)}
								onclick={() => toggleAudience(option.value)}
							>
								{option.label}
							</Button>
						{/each}
					</div>
					{#if targetAudienceSelected}
						<div class="grid gap-4 rounded-md border bg-muted/20 p-4 md:grid-cols-2">
							<div class="grid gap-2">
								<Label>ระดับชั้น</Label>
								<Select.Root type="single" bind:value={selectedGradeLevelId}>
									<Select.Trigger class="w-full">{selectedGradeLevelLabel}</Select.Trigger>
									<Select.Content>
										<Select.Item value="">ทุกระดับชั้น</Select.Item>
										{#each gradeLevels as gradeLevel (gradeLevel.id)}
											<Select.Item value={gradeLevel.id}>{gradeLevel.name}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
							<div class="grid gap-2">
								<Label>ห้องเรียน</Label>
								<Select.Root type="single" bind:value={selectedClassRoomId}>
									<Select.Trigger class="w-full">{selectedClassroomLabel}</Select.Trigger>
									<Select.Content>
										<Select.Item value="">ทุกห้องเรียน</Select.Item>
										{#each filteredClassrooms as classroom (classroom.id)}
											<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
						</div>
					{/if}
				</section>

				<section class="grid gap-3">
					<Label>การเผยแพร่และแจ้งเตือน</Label>
					<div class="grid gap-3 rounded-md border p-4">
						<label class="flex items-center gap-2 text-sm font-medium">
							<Checkbox bind:checked={isPublic} />
							แสดงในปฏิทินสาธารณะ
						</label>
						<label class="flex items-center gap-2 text-sm font-medium">
							<Checkbox bind:checked={notifyAudience} />
							แจ้งเตือนผู้ชมเมื่อเผยแพร่
						</label>
					</div>
				</section>

				<section class="grid gap-3">
					<Label>แจ้งเตือนล่วงหน้า</Label>
					<div class="grid gap-3 rounded-md border p-4">
						<div class="grid gap-3 sm:grid-cols-3">
							<label class="flex items-center gap-2 text-sm font-medium">
								<Checkbox bind:checked={reminder1Day} />
								1 วัน
							</label>
							<label class="flex items-center gap-2 text-sm font-medium">
								<Checkbox bind:checked={reminder3Days} />
								3 วัน
							</label>
							<label class="flex items-center gap-2 text-sm font-medium">
								<Checkbox bind:checked={reminder7Days} />
								7 วัน
							</label>
						</div>
						<div class="grid max-w-xs gap-2">
							<Label for="calendar-event-custom-reminder">กำหนดเอง (วัน)</Label>
							<Input
								id="calendar-event-custom-reminder"
								type="number"
								min="1"
								step="1"
								bind:value={customReminderDays}
							/>
						</div>
					</div>
				</section>
			</div>

			<Dialog.Footer class="sticky bottom-0 mt-6 border-t bg-background py-4">
				<Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังบันทึก..."
					disabled={hasMultipleTargetRows || !title.trim() || !startDate || !endDate}
					class={cn('min-w-36')}
				>
					บันทึกและเผยแพร่
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
