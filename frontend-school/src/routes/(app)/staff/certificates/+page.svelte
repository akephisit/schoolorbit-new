<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import {
		listCertificateCampaigns,
		type CertificateCampaignSummary
	} from '#lib/api/certificates.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import CertificateCampaignList from '#lib/components/certificates/CertificateCampaignList.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Plus } from '@lucide/svelte';

	const canReadCampaigns = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canCreateCampaign = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
		)
	);

	let campaigns: CertificateCampaignSummary[] = $state.raw([]);
	let loading = $state(true);
	let error = $state(''),
		loaded = $state(false);
	let { data }: PageProps = $props();
	const source = $derived(data.campaigns),
		request = new LatestRequest();
	const identity = $derived.by(() => {
		void $authStore.user;
		void $can;
		return appIdentityKey();
	});
	let owner = $state(''),
		ownerEpoch = $state(0),
		disposed = false,
		consumed: typeof data.campaigns | null = null;
	$effect.pre(() => {
		const key = identity,
			operation = source,
			allowed = canReadCampaigns;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				request.abort();
				campaigns = [];
				loaded = false;
				loading = allowed;
				error = '';
			}
			if (!allowed || operation === consumed) return;
			consumed = operation;
			const t = request.begin();
			loading = true;
			error = '';
			void operation.then((r) => applyCampaigns(r, t.revision));
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		request.abort();
	});
	function applyCampaigns(r: Awaited<typeof data.campaigns>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (!r.ok) {
			error = r.error;
			return;
		}
		if (r.data.identityKey !== identity) return;
		campaigns = r.data.records ?? [];
		loaded = true;
	}

	async function loadCampaigns() {
		if (disposed || !canReadCampaigns) return;
		const identityKey = identity,
			t = request.begin();
		loading = true;
		error = '';
		applyCampaigns(
			await captureRouteLoad(
				listCertificateCampaigns({}, { signal: t.signal }).then((records) => ({
					identityKey,
					records
				})),
				'โหลดชุดออกเกียรติบัตรไม่สำเร็จ'
			),
			t.revision
		);
	}

	function removePurgedCampaign(campaignId: string): void {
		if (disposed || !canReadCampaigns) return;
		request.abort();
		loading = false;
		campaigns = campaigns.filter((campaign) => campaign.id !== campaignId);
	}
</script>

<PageShell
	title="ชุดออกเกียรติบัตร"
	description="จัดการกิจกรรม แม่แบบ รายชื่อ และคำขอออกเกียรติบัตรตามขอบเขตหน่วยงาน"
>
	{#snippet actions()}
		{#if canCreateCampaign}
			<Button href="/staff/certificates/new" data-sveltekit-preload-data="tap">
				<Plus class="size-4" />
				สร้างกิจกรรม
			</Button>
		{/if}
	{/snippet}

	{#if !canReadCampaigns}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูชุดออกเกียรติบัตร"
			description="ต้องมีสิทธิ์อ่านระดับหน่วยงานหรือระดับโรงเรียน จึงจะเปิดพื้นที่จัดการนี้ได้"
		/>
	{:else}
		<section data-testid="certificate-campaigns">
			{#if loading && loaded}<p role="status">กำลังอัปเดตกิจกรรม</p>{/if}
			{#if error}
				<PageState
					variant="error"
					title="โหลดชุดออกเกียรติบัตรไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadCampaigns}
				/>
			{/if}
			{#if loading && !loaded}<div role="status" aria-label="กำลังโหลดกิจกรรมเกียรติบัตร">
					<PageSkeleton variant="cards" rows={4} />
				</div>
			{:else if loaded}
				{#key owner}
					{@const childOwner = ownerEpoch}
					<CertificateCampaignList
						{campaigns}
						canCreate={canCreateCampaign}
						onpurged={(id) => {
							if (childOwner === ownerEpoch) removePurgedCampaign(id);
						}}
					/>
				{/key}
			{/if}
		</section>
	{/if}
</PageShell>
