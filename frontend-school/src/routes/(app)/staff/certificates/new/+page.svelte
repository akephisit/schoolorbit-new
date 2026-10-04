<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import {
		createCertificateCampaign,
		listCertificateOwnerOptions,
		type CreateCertificateCampaignRequest
	} from '#lib/api/certificates.js';
	import {
		lookupAcademicYears,
		type AcademicYearLookupItem,
		type OrganizationUnitLookupItem
	} from '#lib/api/lookup.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import CertificateCampaignForm, {
		type CertificateCampaignFormValue
	} from '#lib/components/certificates/CertificateCampaignForm.svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { toast } from 'svelte-sonner';

	const canCreateOrganizationCampaign = $derived(
		$can.has(PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT)
	);
	const canCreateSchoolCampaign = $derived($can.has(PERMISSIONS.CERTIFICATE_CREATE_SCHOOL));
	const canCreateCampaign = $derived(canCreateOrganizationCampaign || canCreateSchoolCampaign);

	let { data }: PageProps = $props();
	let academicYears: AcademicYearLookupItem[] = $state.raw([]),
		ownerOptions: OrganizationUnitLookupItem[] = $state.raw([]);
	let yearsLoading = $state(true),
		ownersLoading = $state(true),
		yearsLoaded = $state(false),
		ownersLoaded = $state(false);
	let yearsError = $state(''),
		ownersError = $state(''),
		saving = $state(false);
	const yearsRequest = new LatestRequest(),
		ownersRequest = new LatestRequest();
	const identity = $derived.by(() => {
		void $authStore.user;
		void $can;
		return appIdentityKey();
	});
	let owner = $state(''),
		ownerEpoch = 0,
		disposed = false;
	let consumedYears: typeof data.years | null = null,
		consumedOwners: typeof data.owners | null = null;
	$effect.pre(() => {
		const key = identity,
			years = data.years,
			owners = data.owners,
			allowed = canCreateCampaign;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				yearsRequest.abort();
				ownersRequest.abort();
				academicYears = [];
				ownerOptions = [];
				yearsLoaded = ownersLoaded = false;
				yearsLoading = ownersLoading = allowed;
				yearsError = ownersError = '';
				saving = false;
			}
			if (!allowed) return;
			if (years !== consumedYears) {
				consumedYears = years;
				const t = yearsRequest.begin();
				yearsLoading = true;
				yearsError = '';
				void years.then((r) => applyYears(r, t.revision));
			}
			if (owners !== consumedOwners) {
				consumedOwners = owners;
				const t = ownersRequest.begin();
				ownersLoading = true;
				ownersError = '';
				void owners.then((r) => applyOwners(r, t.revision));
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		yearsRequest.abort();
		ownersRequest.abort();
	});
	function applyYears(r: Awaited<typeof data.years>, revision: number) {
		if (!yearsRequest.isCurrent(revision)) return;
		yearsLoading = false;
		if (!r.ok) {
			yearsError = r.error;
			return;
		}
		if (r.data.identityKey !== identity) return;
		academicYears = r.data.records ?? [];
		yearsLoaded = true;
	}
	function applyOwners(r: Awaited<typeof data.owners>, revision: number) {
		if (!ownersRequest.isCurrent(revision)) return;
		ownersLoading = false;
		if (!r.ok) {
			ownersError = r.error;
			return;
		}
		if (r.data.identityKey !== identity) return;
		ownerOptions = r.data.records ?? [];
		ownersLoaded = true;
	}
	async function retryYears() {
		if (disposed || !canCreateCampaign) return;
		const identityKey = identity,
			t = yearsRequest.begin();
		yearsLoading = true;
		yearsError = '';
		applyYears(
			await captureRouteLoad(
				lookupAcademicYears({ activeOnly: false }, { signal: t.signal }).then((records) => ({
					identityKey,
					records
				})),
				'โหลดปีการศึกษาไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function retryOwners() {
		if (disposed || !canCreateCampaign) return;
		const identityKey = identity,
			t = ownersRequest.begin();
		ownersLoading = true;
		ownersError = '';
		applyOwners(
			await captureRouteLoad(
				listCertificateOwnerOptions({ signal: t.signal }).then((records) => ({
					identityKey,
					records
				})),
				'โหลดหน่วยงานเจ้าของไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function handleCreate(value: CertificateCampaignFormValue) {
		if (!canCreateCampaign || saving || !yearsLoaded || !ownersLoaded || yearsError || ownersError)
			return;
		const epoch = ownerEpoch,
			key = identity;
		const current = () =>
			!disposed && epoch === ownerEpoch && key === identity && canCreateCampaign;
		saving = true;
		try {
			const payload: CreateCertificateCampaignRequest = {
				academicYearId: value.academicYearId,
				ownerOrganizationUnitId: value.ownerOrganizationUnitId,
				name: value.name,
				eventDate: value.eventDate
			};
			const campaign = await createCertificateCampaign(payload);
			if (!current()) return;
			toast.success('สร้างกิจกรรมเกียรติบัตรแล้ว');
			await goto(resolve(`staff/certificates/${campaign.id}/overview`));
		} catch (createError) {
			if (!current()) return;
			toast.error(createError instanceof Error ? createError.message : 'ไม่สามารถสร้างกิจกรรมได้');
		} finally {
			if (current()) saving = false;
		}
	}
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
	{:else}
		<section data-testid="campaign-years" aria-busy={yearsLoading}>
			{#if yearsError}<PageState
					variant="error"
					title="โหลดปีการศึกษาไม่สำเร็จ"
					description={yearsError}
					actionLabel="ลองโหลดปีการศึกษาอีกครั้ง"
					onaction={retryYears}
				/>{/if}
			{#if yearsLoading && !yearsLoaded}<div role="status" aria-label="กำลังโหลดปีการศึกษา">
					<PageSkeleton variant="form" rows={1} />
				</div>
			{:else if yearsLoaded && !academicYears.length}<PageState
					title="ยังไม่มีปีการศึกษา"
					description="เพิ่มปีการศึกษาก่อนสร้างกิจกรรมเกียรติบัตร"
				/>{/if}
		</section>
		<section data-testid="campaign-owners" aria-busy={ownersLoading}>
			{#if ownersError}<PageState
					variant="error"
					title="โหลดหน่วยงานเจ้าของไม่สำเร็จ"
					description={ownersError}
					actionLabel="ลองโหลดหน่วยงานอีกครั้ง"
					onaction={retryOwners}
				/>{/if}
			{#if ownersLoading && !ownersLoaded}<div role="status" aria-label="กำลังโหลดหน่วยงานเจ้าของ">
					<PageSkeleton variant="form" rows={1} />
				</div>{/if}
		</section>
		{#key owner}
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
						yearsReady={yearsLoaded && !yearsError}
						ownersReady={ownersLoaded && !ownersError}
						disabled={!academicYears.length}
						{saving}
						onsubmit={handleCreate}
						oncancel={() => goto(resolve('staff/certificates'))}
					/>
				</Card.Content>
			</Card.Root>
		{/key}
	{/if}
</PageShell>
