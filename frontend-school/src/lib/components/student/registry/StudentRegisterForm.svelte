<script lang="ts">
	import type { Snippet } from 'svelte';
	import {
		studentParentDraft,
		REGISTRATION_STATUS_LABELS,
		registrationStatus,
		type StudentRegisterDraft,
		type StudentRegisterMode
	} from '#lib/forms/student-register.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import StudentParentFields from './StudentParentFields.svelte';
	import { Plus, Save, Trash2 } from '@lucide/svelte';
	let {
		value = $bindable(),
		mode,
		errors = {},
		saving = false,
		disabled = false,
		allowNationalId = false,
		cancelHref,
		formError = '',
		additionalFields,
		guardians,
		onsubmit
	}: {
		value: StudentRegisterDraft;
		mode: StudentRegisterMode;
		errors?: Record<string, string>;
		saving?: boolean;
		disabled?: boolean;
		allowNationalId?: boolean;
		cancelHref: string;
		formError?: string;
		additionalFields?: Snippet;
		guardians?: Snippet;
		onsubmit: () => void;
	} = $props();
	const pending = $derived(disabled || saving);
	const genderLabels: Record<string, string> = { male: 'ชาย', female: 'หญิง', other: 'อื่น ๆ' };
	const nameFields = [
		{ key: 'title', label: 'คำนำหน้า', required: false },
		{ key: 'first_name', label: 'ชื่อ', required: true },
		{ key: 'last_name', label: 'นามสกุล', required: true }
	] as const;
	const contactFields = [
		{ key: 'email', label: 'อีเมล', type: 'email', autocomplete: 'email' },
		{ key: 'phone', label: 'เบอร์โทรศัพท์', type: 'tel', autocomplete: 'tel' }
	] as const;
	function parentErrors(index: number): Record<string, string> {
		const prefix = `parents.${index}.`;
		return Object.fromEntries(
			Object.entries(errors)
				.filter(([key]) => key.startsWith(prefix))
				.map(([key, error]) => [key.slice(prefix.length), error])
		);
	}
</script>

{#snippet fieldError(field: string)}
	{#if errors[field]}
		<p id={`student-${field}-error`} class="text-sm text-destructive">{errors[field]}</p>
	{/if}
{/snippet}

<form
	class="max-w-4xl space-y-6"
	novalidate
	aria-label={mode === 'create' ? 'เพิ่มทะเบียนนักเรียน' : 'แก้ไขทะเบียนนักเรียน'}
	aria-busy={saving}
	onsubmit={(event) => {
		event.preventDefault();
		if (!pending) onsubmit();
	}}
>
	<Card.Root>
		<Card.Header>
			<Card.Title><h2>ทะเบียนและข้อมูลส่วนตัว</h2></Card.Title>
			<Card.Description
				>ข้อมูลประจำตัวและสถานะทะเบียนกลาง การจัดชั้นและห้องอยู่ในทะเบียนรายปี</Card.Description
			>
		</Card.Header>
		<Card.Content class="grid min-w-0 gap-4 sm:grid-cols-2">
			<div class="grid min-w-0 gap-4 sm:col-span-2 sm:grid-cols-3">
				<div class="min-w-0 space-y-2">
					<Label for="student-student_id"
						>รหัสนักเรียน{#if mode === 'create'}<span class="text-destructive" aria-hidden="true"
								>*</span
							>{/if}</Label
					>
					{#if mode === 'create'}
						<Input
							id="student-student_id"
							bind:value={value.student_id}
							disabled={pending}
							required
							aria-invalid={Boolean(errors.student_id)}
							aria-describedby={errors.student_id ? 'student-student_id-error' : undefined}
						/>
					{:else}
						<p id="student-student_id" class="break-words font-medium">{value.student_id || '—'}</p>
					{/if}
					{@render fieldError('student_id')}
				</div>
				<div class="min-w-0 space-y-2">
					<Label for="student-registration_status">สถานะทะเบียน</Label>
					<Select.Root
						type="single"
						value={value.registration_status}
						onValueChange={(status) => {
							const selected = registrationStatus(status);
							if (selected) value.registration_status = selected;
						}}
						disabled={pending}
					>
						<Select.Trigger id="student-registration_status" class="w-full"
							>{REGISTRATION_STATUS_LABELS[value.registration_status]}</Select.Trigger
						>
						<Select.Content
							>{#each Object.entries(REGISTRATION_STATUS_LABELS) as [code, label] (code)}<Select.Item
									value={code}>{label}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<div class="min-w-0 space-y-2">
					<Label for="student-enrollment_date">วันที่เข้าเรียน</Label>
					<DatePicker
						id="student-enrollment_date"
						value={value.enrollment_date}
						onValueChange={(date) => (value.enrollment_date = date ?? '')}
						disabled={pending}
						clearable
						ariaLabel="วันที่เข้าเรียน"
					/>
					{@render fieldError('enrollment_date')}
				</div>
			</div>
			<div class="grid min-w-0 gap-4 sm:col-span-2 sm:grid-cols-5">
				{#each nameFields as field (field.key)}
					<div
						class={field.key === 'title' ? 'min-w-0 space-y-2' : 'min-w-0 space-y-2 sm:col-span-2'}
					>
						<Label for={`student-${field.key}`}
							>{field.label}{#if field.required}<span class="text-destructive" aria-hidden="true"
									>*</span
								>{/if}</Label
						>
						<Input
							id={`student-${field.key}`}
							bind:value={value[field.key]}
							disabled={pending}
							required={field.required}
							aria-invalid={Boolean(errors[field.key])}
							aria-describedby={errors[field.key] ? `student-${field.key}-error` : undefined}
						/>
						{@render fieldError(field.key)}
					</div>
				{/each}
			</div>
			<div class="min-w-0 space-y-2">
				<Label for="student-date_of_birth">วันเกิด</Label>
				<DatePicker
					id="student-date_of_birth"
					value={value.date_of_birth}
					onValueChange={(date) => (value.date_of_birth = date ?? '')}
					disabled={pending}
					clearable
					ariaLabel="วันเกิด"
				/>
				{@render fieldError('date_of_birth')}
			</div>
			<div class="min-w-0 space-y-2">
				<Label for="student-gender">เพศ</Label>
				<Select.Root
					type="single"
					value={value.gender || '__unspecified'}
					onValueChange={(gender) => (value.gender = gender === '__unspecified' ? '' : gender)}
					disabled={pending}
				>
					<Select.Trigger id="student-gender" class="w-full"
						>{genderLabels[value.gender] ?? (value.gender || 'ยังไม่ระบุ')}</Select.Trigger
					>
					<Select.Content>
						<Select.Item value="__unspecified">ยังไม่ระบุ</Select.Item>
						{#each Object.entries(genderLabels) as [code, label] (code)}<Select.Item value={code}
								>{label}</Select.Item
							>{/each}
					</Select.Content>
				</Select.Root>
				{@render fieldError('gender')}
			</div>
			{#if mode === 'create' && allowNationalId}
				<div class="min-w-0 space-y-2">
					<Label for="student-national_id">เลขบัตรประชาชน</Label>
					<Input
						id="student-national_id"
						bind:value={value.national_id}
						inputmode="numeric"
						maxlength={13}
						disabled={pending}
						aria-invalid={Boolean(errors.national_id)}
						aria-describedby={errors.national_id ? 'student-national_id-error' : undefined}
					/>
					{@render fieldError('national_id')}
				</div>
			{/if}
			{@render additionalFields?.()}
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header
			><Card.Title><h2>ข้อมูลติดต่อ</h2></Card.Title><Card.Description
				>ระบุเฉพาะข้อมูลที่มี</Card.Description
			></Card.Header
		>
		<Card.Content class="grid min-w-0 gap-4 sm:grid-cols-2">
			{#each contactFields as field (field.key)}
				<div class="min-w-0 space-y-2">
					<Label for={`student-${field.key}`}>{field.label}</Label>
					<Input
						id={`student-${field.key}`}
						type={field.type}
						autocomplete={field.autocomplete}
						bind:value={value[field.key]}
						disabled={pending}
						aria-invalid={Boolean(errors[field.key])}
						aria-describedby={errors[field.key] ? `student-${field.key}-error` : undefined}
					/>
					{@render fieldError(field.key)}
				</div>
			{/each}
			<div class="min-w-0 space-y-2 sm:col-span-2">
				<Label for="student-address">ที่อยู่</Label>
				<Textarea
					id="student-address"
					bind:value={value.address}
					autocomplete="street-address"
					disabled={pending}
					aria-invalid={Boolean(errors.address)}
					aria-describedby={errors.address ? 'student-address-error' : undefined}
				/>
				{@render fieldError('address')}
			</div>
		</Card.Content>
	</Card.Root>

	{#if mode === 'create'}
		<Card.Root>
			<Card.Header
				><Card.Title><h2>บัญชีผู้ใช้</h2></Card.Title><Card.Description
					>กำหนดรหัสผ่านสำหรับเข้าสู่ระบบ</Card.Description
				></Card.Header
			>
			<Card.Content class="grid min-w-0 gap-4 sm:grid-cols-2">
				{#each [{ key: 'password', label: 'รหัสผ่าน' }, { key: 'confirmPassword', label: 'ยืนยันรหัสผ่าน' }] as field (field.key)}
					<div class="min-w-0 space-y-2">
						<Label for={`student-${field.key}`}
							>{field.label}<span class="text-destructive" aria-hidden="true">*</span></Label
						>
						<Input
							id={`student-${field.key}`}
							type="password"
							autocomplete="new-password"
							bind:value={value[field.key as 'password' | 'confirmPassword']}
							disabled={pending}
							required
							aria-invalid={Boolean(errors[field.key])}
							aria-describedby={errors[field.key]
								? `student-${field.key}-error`
								: 'student-password-help'}
						/>
						{@render fieldError(field.key)}
					</div>
				{/each}
				<p id="student-password-help" class="text-sm text-muted-foreground sm:col-span-2">
					รหัสผ่าน 8–128 ตัวอักษร และไม่เกิน 71 ไบต์
				</p>
			</Card.Content>
		</Card.Root>
		<Card.Root>
			<Card.Header>
				<div class="flex items-center justify-between gap-4">
					<div class="space-y-2">
						<Card.Title><h2>ผู้ปกครอง</h2></Card.Title><Card.Description
							>เพิ่มพร้อมนักเรียน หรือเชื่อมโยงภายหลัง</Card.Description
						>
					</div>
					<Switch
						id="student-parents-enabled"
						checked={value.parentsEnabled}
						disabled={pending}
						aria-label="เพิ่มผู้ปกครองพร้อมนักเรียน"
						onCheckedChange={(enabled) => {
							value.parentsEnabled = enabled;
							if (enabled && !value.parents.length) value.parents = [studentParentDraft()];
						}}
					/>
				</div>
			</Card.Header>
			{#if value.parentsEnabled}
				<Card.Content class="space-y-4">
					{#each value.parents as _parent, index (index)}
						<section
							class="space-y-4 rounded-lg border p-4"
							aria-label={`ผู้ปกครองคนที่ ${index + 1}`}
						>
							<div class="flex items-center justify-between gap-3">
								<h3 class="font-medium">
									ผู้ปกครองคนที่ {index + 1}{index === 0 ? ' (หลัก)' : ''}
								</h3>
								<Button
									type="button"
									variant="ghost"
									size="icon"
									disabled={pending}
									aria-label={`ลบผู้ปกครองคนที่ ${index + 1}`}
									onclick={() =>
										(value.parents = value.parents.filter((_, position) => position !== index))}
									><Trash2 aria-hidden="true" /></Button
								>
							</div>
							<StudentParentFields
								bind:value={value.parents[index]}
								idPrefix={`student-parent-${index}`}
								errors={parentErrors(index)}
								disabled={pending}
								{allowNationalId}
							/>
						</section>
					{/each}
					{@render fieldError('parents')}
					<Button
						type="button"
						variant="outline"
						disabled={pending || value.parents.length >= 10}
						onclick={() => (value.parents = [...value.parents, studentParentDraft()])}
						><Plus aria-hidden="true" />เพิ่มผู้ปกครอง</Button
					>
				</Card.Content>
			{/if}
		</Card.Root>
	{:else}
		{@render guardians?.()}
	{/if}
	{#if formError}<p
			class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
			role="alert"
		>
			{formError}
		</p>{/if}
	<div class="flex flex-wrap justify-end gap-3">
		<Button href={cancelHref} variant="outline" disabled={saving}>ยกเลิก</Button>
		<LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก..." {disabled}
			><Save aria-hidden="true" />{mode === 'create'
				? 'เพิ่มนักเรียน'
				: 'บันทึกการแก้ไข'}</LoadingButton
		>
	</div>
</form>
