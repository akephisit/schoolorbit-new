<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import {
		createCertificateCampaign,
		listCertificateOwnerOptions,
		type CreateCertificateCampaignRequest
	} from '$lib/api/certificates';
	import {
		lookupAcademicYears,
		type AcademicYearLookupItem,
		type OrganizationUnitLookupItem
	} from '$lib/api/lookup';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateCampaignForm, {
		type CertificateCampaignFormValue
	} from '$lib/components/certificates/CertificateCampaignForm.svelte';
	import * as Card from '$lib/components/ui/card';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { toast } from 'svelte-sonner';

	const canCreateOrganizationCampaign = $derived(
		$can.has(PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT)
	);
	const canCreateSchoolCampaign = $derived($can.has(PERMISSIONS.CERTIFICATE_CREATE_SCHOOL));
	const canCreateCampaign = $derived(canCreateOrganizationCampaign || canCreateSchoolCampaign);

	let academicYears: AcademicYearLookupItem[] = $state.raw([]);
	let ownerOptions: OrganizationUnitLookupItem[] = $state.raw([]);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');

	async function loadFormOptions() {
		if (!canCreateCampaign) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			[academicYears, ownerOptions] = await Promise.all([
				lookupAcademicYears({ activeOnly: false }),
				listCertificateOwnerOptions()
			]);
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดข้อมูลอ้างอิงได้';
		} finally {
			loading = false;
		}
	}

	async function handleCreate(value: CertificateCampaignFormValue) {
		if (!canCreateCampaign || saving) return;
		saving = true;
		try {
			const payload: CreateCertificateCampaignRequest = {
				academicYearId: value.academicYearId,
				ownerOrganizationUnitId: value.ownerOrganizationUnitId,
				name: value.name,
				eventDate: value.eventDate
			};
			const campaign = await createCertificateCampaign(payload);
			toast.success('สร้างกิจกรรมเกียรติบัตรแล้ว');
			await goto(resolve(`/staff/certificates/${campaign.id}/overview`));
		} catch (createError) {
			toast.error(createError instanceof Error ? createError.message : 'ไม่สามารถสร้างกิจกรรมได้');
		} finally {
			saving = false;
		}
	}

	onMount(loadFormOptions);
</script>

<PageShell
	title="สร้างกิจกรรมเกียรติบัตร"
	description="กำหนดข้อมูลร่วมก่อนเพิ่มแม่แบบและนำเข้ารายชื่อผู้รับ"
	backHref="/staff/certificates"
>
	{#if !canCreateCampaign}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์สร้างกิจกรรม"
			description="ต้องมีสิทธิ์สร้างระดับหน่วยงานหรือระดับโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="form" rows={4} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดแบบฟอร์มไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadFormOptions}
		/>
	{:else if academicYears.length === 0}
		<PageState
			title="ยังไม่มีปีการศึกษา"
			description="เพิ่มปีการศึกษาก่อนสร้างกิจกรรมเกียรติบัตร"
		/>
	{:else}
		<Card.Root class="mx-auto w-full max-w-4xl">
			<Card.Header class="border-b">
				<Card.Title>ข้อมูลกิจกรรม</Card.Title>
				<Card.Description>
					เลขลำดับกิจกรรมจะถูกจองเมื่อออกเกียรติบัตรครั้งแรก ไม่ได้จองในขั้นตอนนี้
				</Card.Description>
			</Card.Header>
			<Card.Content>
				<CertificateCampaignForm
					{academicYears}
					{ownerOptions}
					allowSchoolOwner={canCreateSchoolCampaign}
					{saving}
					onsubmit={handleCreate}
					oncancel={() => goto(resolve('/staff/certificates'))}
				/>
			</Card.Content>
		</Card.Root>
	{/if}
</PageShell>
