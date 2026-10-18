<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getLeaveRoutingOptions,
		saveLeavePolicy,
		type LeaveType,
		type LeavePolicyVersion,
		type LeaveRoutingOptions,
		type LeaveUnit,
		type ProcessTemplateStep,
		type SaveLeavePolicyRequest
	} from '#lib/api/staff-leave.js';
	import { PERSONNEL_TYPE_LABELS, type StaffPersonnelType } from '#lib/forms/staff-career.js';
	import {
		policyLimitInput,
		leavePolicyDraft,
		newLeavePolicyStep
	} from '#lib/staff-leave/drafts.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { LoadingButton, PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Choice from './Choice.svelte';
	let {
		type,
		initialPolicy,
		oncancel,
		onsaved
	}: {
		type: LeaveType;
		initialPolicy?: LeavePolicyVersion;
		oncancel: () => void;
		onsaved: (policy: LeavePolicyVersion) => void;
	} = $props();
	const initial = untrack(() => leavePolicyDraft(type.kind, initialPolicy));
	let effectiveFrom = $state<string | undefined>(initial.effectiveFrom),
		sourceReference = $state(initial.sourceReference),
		personnelTypes = $state<StaffPersonnelType[]>(initial.personnelTypes);
	let unit = $state<LeaveUnit>(initial.unit);
	let countingMode = $state<'calendar_days' | 'working_days'>(initial.countingMode),
		weekdays = $state<number[]>(initial.weekdays),
		excludedDates = $state<string[]>(initial.excludedDates),
		dateToExclude = $state<string | undefined>();
	let quotaMonth = $state(initial.quotaMonth),
		maxPerRequest = $state(initial.maxPerRequest),
		annualLimit = $state(initial.annualLimit),
		evidenceRequired = $state(initial.evidenceRequired),
		evidenceFromAmount = $state(initial.evidenceFromAmount),
		minimumNoticeDays = $state(initial.minimumNoticeDays);
	let steps = $state<ProcessTemplateStep[]>(initial.steps),
		targets = $state<string[]>([]);
	let enabled = $state(initial.enabled);
	let options = $state<LeaveRoutingOptions | null>(null),
		optionsLoading = $state(true),
		optionsError = $state(''),
		error = $state(''),
		pending = $state(false),
		disposed = false;
	const controller = new AbortController(),
		intent = new LeaveMutationIntent();
	const amountLabel = $derived(unit === 'minute' ? 'นาที' : 'วัน');
	const amountStep = $derived(unit === 'half_day' ? '0.5' : '1');
	const recipientOptions = $derived(
		options
			? [
					...options.users.map((user) => ({ value: `user:${user.id}`, label: user.displayName })),
					...options.organizations.map((org) => ({
						value: `organization_unit:${org.id}`,
						label: `หน่วยงาน: ${org.name}`
					})),
					...options.positions.map((position) => ({
						value: `organization_position:${position.organizationUnitId}:${position.code}`,
						label: `ตำแหน่ง: ${position.name} · ${options?.organizations.find((org) => org.id === position.organizationUnitId)?.name ?? ''}`
					}))
				]
			: []
	);
	async function loadOptions() {
		optionsLoading = true;
		optionsError = '';
		try {
			const result = await getLeaveRoutingOptions({ signal: controller.signal });
			if (!disposed) options = result;
		} catch (cause) {
			if (!disposed)
				optionsError = cause instanceof Error ? cause.message : 'โหลดผู้พิจารณาไม่สำเร็จ';
		} finally {
			if (!disposed) optionsLoading = false;
		}
	}
	$effect.pre(() => {
		untrack(() => {
			void loadOptions();
		});
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
	});
	function toggleCategory(value: StaffPersonnelType, checked: boolean) {
		personnelTypes = checked
			? [...personnelTypes, value]
			: personnelTypes.filter((type) => type !== value);
	}
	function toggleDay(value: number, checked: boolean) {
		weekdays = checked ? [...weekdays, value].sort() : weekdays.filter((day) => day !== value);
	}
	function addRecipient(index: number) {
		const selected = targets[index];
		if (!selected) return;
		const [kind, id, positionCode] = selected.split(':');
		const recipient =
			kind === 'user'
				? { kind: 'user' as const, userId: id }
				: kind === 'organization_unit'
					? { kind: 'organization_unit' as const, organizationUnitId: id }
					: { kind: 'organization_position' as const, organizationUnitId: id, positionCode };
		const current = steps[index].recipients ?? [];
		if (
			!current.some(
				(value) =>
					value.kind === recipient.kind &&
					value.userId === recipient.userId &&
					value.organizationUnitId === recipient.organizationUnitId &&
					value.positionCode === recipient.positionCode
			)
		)
			steps[index].recipients = [...current, recipient];
		targets[index] = '';
	}
	function recipientLabel(
		recipient: NonNullable<ProcessTemplateStep['recipients']>[number]
	): string {
		if (recipient.kind === 'user')
			return (
				options?.users.find((user) => user.id === recipient.userId)?.displayName ??
				'บุคลากรที่เลือก'
			);
		if (recipient.kind === 'organization_unit')
			return `หน่วยงาน: ${options?.organizations.find((org) => org.id === recipient.organizationUnitId)?.name ?? 'หน่วยงานที่เลือก'}`;
		return `ตำแหน่ง: ${options?.positions.find((position) => position.organizationUnitId === recipient.organizationUnitId && position.code === recipient.positionCode)?.name ?? 'ตำแหน่งที่เลือก'}`;
	}
	function move(index: number, direction: number) {
		const next = index + direction;
		if (next < 0 || next >= steps.length) return;
		const copy = [...steps];
		[copy[index], copy[next]] = [copy[next], copy[index]];
		steps = copy;
		targets = [];
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		error = '';
		if (!effectiveFrom || !sourceReference.trim() || !personnelTypes.length) {
			error = 'กรอกวันที่มีผล แหล่งอ้างอิง และเลือกประเภทบุคลากร';
			return;
		}
		if (countingMode === 'working_days' && !weekdays.length) {
			error = 'เลือกวันทำงานอย่างน้อยหนึ่งวัน';
			return;
		}
		if (!steps.length || steps.some((step) => !step.title.trim() || !step.recipients?.length)) {
			error = 'ตั้งชื่อขั้นตอนและเพิ่มผู้พิจารณาทุกขั้นตอน';
			return;
		}
		if (
			minimumNoticeDays !== '' &&
			(!Number.isInteger(Number(minimumNoticeDays)) ||
				Number(minimumNoticeDays) < 0 ||
				Number(minimumNoticeDays) > 365)
		) {
			error = 'จำนวนวันที่ยื่นล่วงหน้าต้องอยู่ระหว่าง 0 ถึง 365';
			return;
		}
		pending = true;
		try {
			const basis: Omit<SaveLeavePolicyRequest, 'requestId'> = {
				expectedRevision: type.revision,
				effectiveFrom,
				sourceReference: sourceReference.trim(),
				personnelTypes,
				enabled,
				unit,
				countingMode,
				workingWeekdays: countingMode === 'working_days' ? weekdays : [],
				excludedDates: countingMode === 'working_days' ? excludedDates : [],
				quotaYearStartMonth: quotaMonth,
				maxPerRequest: policyLimitInput(maxPerRequest, unit),
				annualLimit: policyLimitInput(annualLimit, unit),
				evidenceRequired,
				evidenceFromAmount: policyLimitInput(evidenceFromAmount, unit),
				minimumNoticeDays: minimumNoticeDays === '' ? null : Number(minimumNoticeDays),
				steps: steps.map((step) => ({ ...step, title: step.title.trim() }))
			};
			const result = await saveLeavePolicy(type.id, { requestId: intent.for(basis), ...basis });
			if (!disposed) onsaved(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกนโยบายไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form onsubmit={save} class="space-y-6" data-testid="leave-policy-editor">
	<section class="space-y-4">
		<h3 class="font-semibold">ขอบเขตและวันที่มีผล</h3>
		<div class="flex items-center gap-2">
			<Checkbox id="policy-enabled" bind:checked={enabled} disabled={pending} /><Label
				for="policy-enabled">เปิดรับคำขอสำหรับประเภทบุคลากรที่เลือกตั้งแต่วันที่มีผล</Label
			>
		</div>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="policy-effective">วันที่มีผล *</Label><DatePicker
					id="policy-effective"
					ariaLabel="วันที่นโยบายมีผล"
					bind:value={effectiveFrom}
					disabled={pending}
				/>
			</div>
			<div class="space-y-2">
				<Label for="policy-source">แหล่งอ้างอิง / เลขคำสั่ง *</Label><Input
					id="policy-source"
					bind:value={sourceReference}
					disabled={pending}
				/>
			</div>
		</div>
		<fieldset class="space-y-3">
			<legend class="mb-2 text-sm font-medium">ประเภทบุคลากรที่ใช้นโยบายนี้ *</legend>
			<div class="grid gap-3 sm:grid-cols-2">
				{#each Object.entries(PERSONNEL_TYPE_LABELS) as [value, label] (value)}<div
						class="flex items-center gap-2"
					>
						<Checkbox
							id={`policy-personnel-${value}`}
							checked={personnelTypes.includes(value as StaffPersonnelType)}
							onCheckedChange={(checked) =>
								toggleCategory(value as StaffPersonnelType, checked === true)}
							disabled={pending}
						/><Label for={`policy-personnel-${value}`}>{label}</Label>
					</div>{/each}
			</div>
		</fieldset>
		<p class="text-xs text-muted-foreground">
			แต่ละประเภทบุคลากรเลือกนโยบายตามวันที่เริ่มคำขอ การบันทึกฉบับใหม่ไม่ลบฉบับเดิม
		</p>
	</section>
	<section class="space-y-4">
		<h3 class="font-semibold">หน่วยและวิธีนับ</h3>
		<div class="grid gap-4 sm:grid-cols-2">
			<Choice
				id="policy-unit"
				label="หน่วย *"
				bind:value={unit}
				options={type.kind === 'exit'
					? [{ value: 'minute', label: 'นาที' }]
					: [
							{ value: 'day', label: 'วันเต็ม' },
							{ value: 'half_day', label: 'วัน (รองรับครึ่งวัน)' }
						]}
				disabled={pending}
			/><Choice
				id="policy-counting"
				label="วิธีนับวัน *"
				bind:value={countingMode}
				options={[
					{ value: 'calendar_days', label: 'วันปฏิทิน' },
					{ value: 'working_days', label: 'วันทำงานที่โรงเรียนกำหนด' }
				]}
				disabled={pending || unit === 'minute'}
			/>
		</div>
		{#if countingMode === 'working_days'}<fieldset>
				<legend class="mb-2 text-sm font-medium">วันทำงาน *</legend>
				<div class="flex flex-wrap gap-3">
					{#each ['จันทร์', 'อังคาร', 'พุธ', 'พฤหัสบดี', 'ศุกร์', 'เสาร์', 'อาทิตย์'] as label, index (label)}<div
							class="flex items-center gap-2"
						>
							<Checkbox
								id={`policy-weekday-${index}`}
								checked={weekdays.includes(index + 1)}
								onCheckedChange={(checked) => toggleDay(index + 1, checked === true)}
								disabled={pending}
							/><Label for={`policy-weekday-${index}`}>{label}</Label>
						</div>{/each}
				</div>
			</fieldset>
			<div class="space-y-2">
				<Label for="policy-excluded-date">วันหยุด / วันที่ไม่นับเพิ่มเติม</Label>
				<div class="flex items-center gap-2">
					<DatePicker
						id="policy-excluded-date"
						ariaLabel="วันที่ไม่นับเพิ่มเติม"
						bind:value={dateToExclude}
						disabled={pending}
					/><Button
						type="button"
						variant="outline"
						disabled={pending || !dateToExclude}
						onclick={() => {
							if (dateToExclude)
								excludedDates = [...new Set([...excludedDates, dateToExclude])].sort();
							dateToExclude = undefined;
						}}>เพิ่มวัน</Button
					>
				</div>
				{#each excludedDates as date (date)}<div
						class="flex items-center justify-between gap-2 rounded-md border p-2"
					>
						<span class="text-sm">{date}</span><Button
							type="button"
							variant="ghost"
							size="sm"
							disabled={pending}
							onclick={() => (excludedDates = excludedDates.filter((value) => value !== date))}
							>นำวันออก</Button
						>
					</div>{/each}
			</div>{/if}
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="policy-max">สูงสุดต่อคำขอ ({amountLabel})</Label><Input
					id="policy-max"
					type="number"
					min={amountStep}
					step={amountStep}
					value={maxPerRequest}
					oninput={(event) => (maxPerRequest = event.currentTarget.value)}
					disabled={pending}
					placeholder="ไม่กำหนด"
				/>{#if type.kind === 'exit'}<p class="text-xs text-muted-foreground">
						120 นาทีเป็นตัวอย่างของโรงเรียน แก้ไขได้
					</p>{/if}
			</div>
			<div class="space-y-2">
				<Label for="policy-annual">สิทธิที่โรงเรียนกำหนดต่อรอบปี ({amountLabel})</Label><Input
					id="policy-annual"
					type="number"
					min={amountStep}
					step={amountStep}
					value={annualLimit}
					oninput={(event) => (annualLimit = event.currentTarget.value)}
					disabled={pending}
					placeholder="ไม่กำหนด"
				/>
			</div>
			<Choice
				id="policy-year-month"
				label="เดือนเริ่มรอบปี *"
				value={String(quotaMonth)}
				onchange={(value) => (quotaMonth = Number(value))}
				options={Array.from({ length: 12 }, (_, index) => ({
					value: String(index + 1),
					label: new Intl.DateTimeFormat('th-TH', { month: 'long' }).format(
						new Date(2026, index, 1)
					)
				}))}
				disabled={pending}
			/>
		</div>
	</section>
	<section class="space-y-4">
		<h3 class="font-semibold">หลักฐานและการยื่นล่วงหน้า</h3>
		<div class="flex items-center gap-2">
			<Checkbox
				id="policy-evidence-required"
				bind:checked={evidenceRequired}
				disabled={pending}
			/><Label for="policy-evidence-required">ต้องแนบหลักฐานทุกคำขอ</Label>
		</div>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="policy-evidence-threshold">ต้องแนบหลักฐานเมื่อขอตั้งแต่ ({amountLabel})</Label
				><Input
					id="policy-evidence-threshold"
					type="number"
					min={amountStep}
					step={amountStep}
					value={evidenceFromAmount}
					oninput={(event) => (evidenceFromAmount = event.currentTarget.value)}
					disabled={pending || evidenceRequired}
					placeholder="ไม่กำหนด"
				/>
			</div>
			<div class="space-y-2">
				<Label for="policy-notice">ยื่นล่วงหน้าอย่างน้อย (วัน)</Label><Input
					id="policy-notice"
					type="number"
					min="0"
					max="365"
					step="1"
					value={minimumNoticeDays}
					oninput={(event) => (minimumNoticeDays = event.currentTarget.value)}
					disabled={pending}
					placeholder="ไม่กำหนด / ย้อนหลังได้"
				/>
			</div>
		</div>
	</section>
	<section class="space-y-4">
		<h3 class="font-semibold">ลำดับผู้พิจารณา *</h3>
		{#if optionsError}<PageState
				variant="error"
				title="โหลดผู้พิจารณาไม่สำเร็จ"
				description={optionsError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadOptions}
			/>{/if}{#if optionsLoading && !options}<PageSkeleton variant="form" />{:else}
			{#each steps as step, index (index)}<fieldset class="space-y-4 rounded-xl border p-3">
					<legend class="px-1 text-sm font-semibold">ขั้นตอน {index + 1}</legend>
					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<Label for={`policy-step-title-${index}`}>ชื่อขั้นตอน *</Label><Input
								id={`policy-step-title-${index}`}
								bind:value={step.title}
								disabled={pending}
								placeholder="เช่น หัวหน้าฝ่ายตรวจสอบ"
							/>
						</div>
						<Choice
							id={`policy-step-kind-${index}`}
							label="หน้าที่ *"
							bind:value={step.kind}
							options={[
								{ value: 'review', label: 'ตรวจสอบ' },
								{ value: 'approve', label: 'อนุมัติ' }
							]}
							disabled={pending}
						/>
					</div>
					<div class="flex items-end gap-2">
						<div class="min-w-0 flex-1">
							<Choice
								id={`policy-step-recipient-${index}`}
								label="เลือกบุคคล / หน่วยงาน / ตำแหน่ง"
								value={targets[index] ?? ''}
								onchange={(value) => (targets[index] = value)}
								options={recipientOptions}
								searchable
								disabled={pending}
							/>
						</div>
						<Button
							type="button"
							variant="outline"
							disabled={pending || !targets[index]}
							onclick={() => addRecipient(index)}>เพิ่มผู้พิจารณา</Button
						>
					</div>
					<ul class="space-y-2">
						{#each step.recipients ?? [] as recipient, recipientIndex (recipientIndex)}<li
								class="flex items-center justify-between gap-2 rounded-md border p-2"
							>
								<span class="min-w-0 break-words text-sm">{recipientLabel(recipient)}</span><Button
									type="button"
									variant="ghost"
									size="sm"
									aria-label={`นำผู้พิจารณา ${recipientLabel(recipient)} ออกจากขั้นตอน ${index + 1}`}
									disabled={pending}
									onclick={() =>
										(step.recipients = step.recipients?.filter(
											(_, current) => current !== recipientIndex
										))}>นำออก</Button
								>
							</li>{/each}
					</ul>
					<Choice
						id={`policy-step-completion-${index}`}
						label="เมื่อมีผู้พิจารณาหลายคน"
						bind:value={step.completionPolicy}
						options={[
							{ value: 'all', label: 'ดำเนินการครบทุกคน' },
							{ value: 'any', label: 'คนใดคนหนึ่งดำเนินการ' }
						]}
						disabled={pending}
					/>
					<div class="flex flex-wrap gap-4">
						<div class="flex items-center gap-2">
							<Checkbox
								id={`policy-step-signature-${index}`}
								bind:checked={step.requiresSignature}
								disabled={pending}
							/><Label for={`policy-step-signature-${index}`}>ใช้ลายมือชื่อในขั้นตอนนี้</Label>
						</div>
						<div class="flex items-center gap-2">
							<Checkbox
								id={`policy-step-comment-${index}`}
								bind:checked={step.requiresComment}
								disabled={pending}
							/><Label for={`policy-step-comment-${index}`}>ต้องมีความคิดเห็น</Label>
						</div>
					</div>
					<div class="flex flex-wrap justify-end gap-2">
						<Button
							type="button"
							variant="ghost"
							size="sm"
							disabled={pending || index === 0}
							onclick={() => move(index, -1)}>เลื่อนขึ้น</Button
						><Button
							type="button"
							variant="ghost"
							size="sm"
							disabled={pending || index === steps.length - 1}
							onclick={() => move(index, 1)}>เลื่อนลง</Button
						><Button
							type="button"
							variant="ghost"
							size="sm"
							disabled={pending || steps.length === 1}
							onclick={() => {
								steps = steps.filter((_, current) => current !== index);
								targets = [];
							}}>ลบขั้นตอน {index + 1}</Button
						>
					</div>
				</fieldset>{/each}<Button
				type="button"
				variant="outline"
				disabled={pending}
				onclick={() => (steps = [...steps, newLeavePolicyStep()])}>เพิ่มขั้นตอน</Button
			>{/if}
	</section>
	{#if error}<PageState variant="error" title="บันทึกนโยบายไม่สำเร็จ" description={error} />{/if}
	<div class="flex flex-wrap justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending} onclick={oncancel}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={!options}
			>บันทึกนโยบายฉบับใหม่</LoadingButton
		>
	</div>
</form>
