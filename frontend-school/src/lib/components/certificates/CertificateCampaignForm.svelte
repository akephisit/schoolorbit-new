<script lang="ts" module>
	import type { CreateCertificateCampaignRequest } from '$lib/api/certificates';

	export type CertificateCampaignFormValue = CreateCertificateCampaignRequest & {
		confirmAffectsIssuedCertificates: boolean;
	};
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import type { CertificateCampaignDetail } from '$lib/api/certificates';
	import type { AcademicYearLookupItem, OrganizationUnitLookupItem } from '$lib/api/lookup';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { AlertCircle, Award, Building2, CalendarDays, GraduationCap, Save } from 'lucide-svelte';

	const SCHOOL_OWNER_VALUE = '__school__';
	type AcademicYearFormOption = Omit<AcademicYearLookupItem, 'status'> & {
		status?: AcademicYearLookupItem['status'];
	};

	let {
		academicYears,
		ownerOptions,
		campaign,
		allowSchoolOwner = false,
		allowOwnerChange = true,
		saving = false,
		submitLabel = campaign ? 'บันทึกการแก้ไข' : 'สร้างกิจกรรม',
		onsubmit,
		oncancel
	}: {
		academicYears: AcademicYearFormOption[];
		ownerOptions: OrganizationUnitLookupItem[];
		campaign?: CertificateCampaignDetail;
		allowSchoolOwner?: boolean;
		allowOwnerChange?: boolean;
		saving?: boolean;
		submitLabel?: string;
		onsubmit: (value: CertificateCampaignFormValue) => void | Promise<void>;
		oncancel?: () => void;
	} = $props();

	const visibleOwnerOptions = $derived(
		ownerOptions
			.filter((unit) => unit.is_active && unit.code.toUpperCase() !== 'SCHOOL')
			.toSorted(
				(left, right) =>
					left.display_order - right.display_order || left.name.localeCompare(right.name, 'th')
			)
	);

	let form = $state(
		untrack(() => ({
			academicYearId:
				campaign?.academicYearId ??
				academicYears.find((year) => year.status === 'active')?.id ??
				academicYears[0]?.id ??
				'',
			ownerValue:
				campaign?.ownerOrganizationUnitId ??
				(allowSchoolOwner ? SCHOOL_OWNER_VALUE : (ownerOptions[0]?.id ?? '')),
			name: campaign?.name ?? '',
			eventDate: campaign?.eventDate ?? '',
			confirmAffectsIssuedCertificates: false
		}))
	);
	let validationError = $state('');

	const hasIssuedCertificates = $derived(
		campaign?.activitySequence !== null && campaign !== undefined
	);
	const canChangeAcademicYear = $derived(
		campaign === undefined || campaign.activitySequence === null
	);
	const canChangeOwner = $derived(
		allowOwnerChange && (campaign === undefined || campaign.activitySequence === null)
	);
	const issuedSharedFieldsChanged = $derived(
		hasIssuedCertificates &&
			campaign !== undefined &&
			(form.name.trim().replace(/\s+/g, ' ') !== campaign.name ||
				form.eventDate !== campaign.eventDate)
	);

	function ownerDepth(unit: OrganizationUnitLookupItem): number {
		let depth = 0;
		let parentId = unit.parent_unit_id;
		const visited = [unit.id];
		while (parentId && depth < 4 && !visited.includes(parentId)) {
			visited.push(parentId);
			const parent = visibleOwnerOptions.find((candidate) => candidate.id === parentId);
			if (!parent) break;
			depth += 1;
			parentId = parent.parent_unit_id;
		}
		return depth;
	}

	function ownerName(ownerId: string): string | undefined {
		return visibleOwnerOptions.find((unit) => unit.id === ownerId)?.name;
	}

	async function handleSubmit(event: SubmitEvent) {
		event.preventDefault();
		validationError = '';
		const name = form.name.trim().replace(/\s+/g, ' ');
		if (!name || !form.academicYearId || !form.eventDate) {
			validationError = 'กรุณากรอกชื่อกิจกรรม ปีการศึกษา และวันที่จัดกิจกรรมให้ครบ';
			return;
		}
		if (
			!allowSchoolOwner &&
			form.ownerValue === SCHOOL_OWNER_VALUE &&
			campaign?.ownerOrganizationUnitId !== null
		) {
			validationError = 'กรุณาเลือกหน่วยงานเจ้าของกิจกรรม';
			return;
		}
		if (issuedSharedFieldsChanged && !form.confirmAffectsIssuedCertificates) {
			validationError = 'กรุณายืนยันว่าการแก้ชื่อหรือวันที่มีผลต่อใบที่ออกแล้ว';
			return;
		}

		await onsubmit({
			academicYearId: form.academicYearId,
			ownerOrganizationUnitId:
				form.ownerValue === SCHOOL_OWNER_VALUE ? null : form.ownerValue || null,
			name,
			eventDate: form.eventDate,
			confirmAffectsIssuedCertificates: form.confirmAffectsIssuedCertificates
		});
	}
</script>

<form class="space-y-6" onsubmit={handleSubmit}>
	{#if validationError}
		<div
			class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			<AlertCircle class="mt-0.5 size-4 shrink-0" />
			<p>{validationError}</p>
		</div>
	{/if}

	<div class="grid gap-5 md:grid-cols-2">
		<div class="space-y-2 md:col-span-2">
			<Label for="certificate-campaign-name">ชื่อกิจกรรม</Label>
			<div class="relative">
				<Award
					class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
				/>
				<Input
					id="certificate-campaign-name"
					bind:value={form.name}
					class="pl-9"
					maxlength={200}
					placeholder="เช่น กิจกรรมวันภาษาไทย"
					required
				/>
			</div>
			<p class="text-xs text-muted-foreground">
				หนึ่งกิจกรรมมีหลายแบบเกียรติบัตรและออกเพิ่มได้หลายรอบ
			</p>
		</div>

		<div class="space-y-2">
			<Label for="certificate-academic-year">ปีการศึกษา</Label>
			<Select.Root type="single" bind:value={form.academicYearId} disabled={!canChangeAcademicYear}>
				<Select.Trigger id="certificate-academic-year" class="w-full">
					<span class="inline-flex items-center gap-2">
						<GraduationCap class="size-4 text-muted-foreground" />
						{academicYears.find((year) => year.id === form.academicYearId)?.name ??
							'เลือกปีการศึกษา'}
					</span>
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year (year.id)}
						<Select.Item value={year.id}>
							{year.name}{year.status === 'active' ? ' · ปีกำลังใช้งาน' : ''}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="space-y-2">
			<Label for="certificate-event-date">วันที่จัดกิจกรรม</Label>
			<div class="relative">
				<CalendarDays class="sr-only" />
				<DatePicker id="certificate-event-date" bind:value={form.eventDate} />
			</div>
		</div>

		<div class="space-y-2 md:col-span-2">
			<Label for="certificate-owner">หน่วยงานเจ้าของกิจกรรม</Label>
			<Select.Root type="single" bind:value={form.ownerValue} disabled={!canChangeOwner}>
				<Select.Trigger id="certificate-owner" class="w-full">
					<span class="inline-flex min-w-0 items-center gap-2">
						<Building2 class="size-4 shrink-0 text-muted-foreground" />
						<span class="truncate">
							{form.ownerValue === SCHOOL_OWNER_VALUE
								? 'กิจกรรมระดับโรงเรียน'
								: (ownerName(form.ownerValue) ??
									campaign?.ownerOrganizationUnitName ??
									'เลือกหน่วยงาน')}
						</span>
					</span>
				</Select.Trigger>
				<Select.Content>
					{#if allowSchoolOwner || campaign?.ownerOrganizationUnitId === null}
						<Select.Item value={SCHOOL_OWNER_VALUE}>กิจกรรมระดับโรงเรียน</Select.Item>
					{/if}
					{#each visibleOwnerOptions as unit (unit.id)}
						<Select.Item value={unit.id}>
							<span style={`padding-left: ${ownerDepth(unit) * 0.75}rem`}>
								{ownerDepth(unit) > 0 ? '↳ ' : ''}{unit.name}
							</span>
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<p class="text-xs text-muted-foreground">
				สิทธิ์ระดับหน่วยงานตรวจจากหน่วยงานตรงตัว โครงสร้างที่เยื้องใช้เพื่อช่วยอ่านเท่านั้น
			</p>
		</div>
	</div>

	{#if issuedSharedFieldsChanged}
		<label
			class="flex cursor-pointer items-start gap-3 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-950"
		>
			<Checkbox bind:checked={form.confirmAffectsIssuedCertificates} class="mt-0.5" />
			<span>
				<strong class="font-medium">ยืนยันการแก้ข้อมูลร่วมของใบที่ออกแล้ว</strong>
				<span class="mt-0.5 block text-xs text-amber-800">
					ชื่อและวันที่ใหม่จะปรากฏในหน้าตรวจสอบและ PDF ที่สร้างใหม่ของทุกใบในกิจกรรมนี้
				</span>
			</span>
		</label>
	{/if}

	<div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
		{#if oncancel}
			<Button type="button" variant="outline" onclick={oncancel} disabled={saving}>ยกเลิก</Button>
		{/if}
		<LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก...">
			<Save class="size-4" />
			{submitLabel}
		</LoadingButton>
	</div>
</form>
