<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		listIssuedCertificates,
		type IssuedCertificateSummary,
		type RevokeCertificateResult
	} from '$lib/api/certificates';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateBatchDownloadDialog from '$lib/components/certificates/CertificateBatchDownloadDialog.svelte';
	import CertificateDownloadButton from '$lib/components/certificates/CertificateDownloadButton.svelte';
	import CertificateRevokeDialog from '$lib/components/certificates/CertificateRevokeDialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import {
		Award,
		Download,
		FileBadge2,
		RefreshCw,
		Search,
		ShieldAlert,
		UsersRound
	} from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';

	let {
		campaignId,
		canRead = false,
		canDownload = false,
		canRevoke = false
	}: {
		campaignId: string;
		canRead?: boolean;
		canDownload?: boolean;
		canRevoke?: boolean;
	} = $props();

	const recipientLabels: Record<IssuedCertificateSummary['recipientType'], string> = {
		student: 'นักเรียน',
		staff: 'บุคลากร',
		external: 'บุคคลภายนอก'
	};

	let certificates = $state.raw<IssuedCertificateSummary[]>([]);
	let loading = $state(true);
	let error = $state('');
	let search = $state('');
	let statusFilter = $state('all');
	let templateFilter = $state('all');
	let selectedCertificateIds = $state.raw<string[]>([]);
	let batchOpen = $state(false);
	let revokeTarget = $state.raw<IssuedCertificateSummary | null>(null);
	let requestedCampaignId = '';
	let loadGeneration = 0;

	const campaignName = $derived(certificates[0]?.campaignName ?? 'กิจกรรมนี้');
	const issuedCount = $derived(
		certificates.filter((certificate) => certificate.status === 'issued').length
	);
	const revokedCount = $derived(certificates.length - issuedCount);
	const linkedRecipientCount = $derived(
		certificates.filter((certificate) => certificate.recipientType !== 'external').length
	);
	const templateOptions = $derived.by(() => {
		const options = new SvelteMap<string, string>();
		for (const certificate of certificates) {
			options.set(certificate.templateId, certificate.templateName);
		}
		return [...options.entries()].sort((left, right) => left[1].localeCompare(right[1], 'th'));
	});
	const filteredCertificates = $derived.by(() => {
		const normalizedSearch = search.trim().toLocaleLowerCase('th');
		return certificates.filter((certificate) => {
			const fullName = `${certificate.title ?? ''}${certificate.firstName} ${certificate.lastName}`
				.trim()
				.toLocaleLowerCase('th');
			const matchesSearch =
				!normalizedSearch ||
				certificate.certificateNumber.toLocaleLowerCase('th').includes(normalizedSearch) ||
				fullName.includes(normalizedSearch);
			const matchesStatus = statusFilter === 'all' || certificate.status === statusFilter;
			const matchesTemplate = templateFilter === 'all' || certificate.templateId === templateFilter;
			return matchesSearch && matchesStatus && matchesTemplate;
		});
	});
	const selectableFilteredCertificates = $derived(
		filteredCertificates.filter(
			(certificate) =>
				canDownload &&
				certificate.status === 'issued' &&
				certificate.capabilities.canDownload === true
		)
	);
	const allFilteredSelected = $derived(
		selectableFilteredCertificates.length > 0 &&
			selectableFilteredCertificates.every((certificate) =>
				selectedCertificateIds.includes(certificate.id)
			)
	);

	function displayName(certificate: IssuedCertificateSummary): string {
		return `${certificate.title ?? ''}${certificate.firstName} ${certificate.lastName}`.trim();
	}

	function formatDate(value: string): string {
		return new Date(`${value}T00:00:00`).toLocaleDateString('th-TH', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
	}

	function canSelect(certificate: IssuedCertificateSummary): boolean {
		return (
			canDownload &&
			certificate.status === 'issued' &&
			certificate.capabilities.canDownload === true
		);
	}

	function toggleCertificate(certificate: IssuedCertificateSummary, checked: boolean) {
		if (!canSelect(certificate)) return;
		selectedCertificateIds = checked
			? Array.from(new Set([...selectedCertificateIds, certificate.id]))
			: selectedCertificateIds.filter((id) => id !== certificate.id);
	}

	function toggleFiltered(checked: boolean) {
		const visibleIds = selectableFilteredCertificates.map((certificate) => certificate.id);
		if (!checked) {
			const visible = new Set(visibleIds);
			selectedCertificateIds = selectedCertificateIds.filter((id) => !visible.has(id));
			return;
		}
		selectedCertificateIds = Array.from(new Set([...selectedCertificateIds, ...visibleIds]));
	}

	function resetCampaignView() {
		certificates = [];
		search = '';
		statusFilter = 'all';
		templateFilter = 'all';
		selectedCertificateIds = [];
		batchOpen = false;
		revokeTarget = null;
		error = '';
	}

	async function loadCertificates(targetCampaignId: string) {
		const generation = ++loadGeneration;
		if (!canRead) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			const loaded = await listIssuedCertificates(targetCampaignId);
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			certificates = loaded;
			const downloadableIds = new Set(
				loaded
					.filter(
						(certificate) =>
							canDownload &&
							certificate.status === 'issued' &&
							certificate.capabilities.canDownload === true
					)
					.map((certificate) => certificate.id)
			);
			selectedCertificateIds = selectedCertificateIds.filter((id) => downloadableIds.has(id));
		} catch (loadError) {
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			error = loadError instanceof Error ? loadError.message : 'โหลดใบที่ออกแล้วไม่สำเร็จ';
		} finally {
			if (generation === loadGeneration && targetCampaignId === campaignId) loading = false;
		}
	}

	function ensureLoaded() {
		if (!campaignId || requestedCampaignId === campaignId) return;
		if (requestedCampaignId !== '' && requestedCampaignId !== campaignId) {
			resetCampaignView();
		}
		requestedCampaignId = campaignId;
		void loadCertificates(campaignId);
	}

	function handleRevoked(result: RevokeCertificateResult) {
		certificates = certificates.map((certificate) =>
			certificate.id === result.certificate.id ? result.certificate : certificate
		);
		selectedCertificateIds = selectedCertificateIds.filter((id) => id !== result.certificate.id);
		revokeTarget = null;
	}

	onMount(ensureLoaded);
	afterNavigate(ensureLoaded);
</script>

<div class="space-y-4">
	{#if canRead}
		<div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3">
			<div class="flex items-center gap-2 text-sm text-muted-foreground">
				<Award class="size-4 text-primary" />
				<span>ทะเบียนกิจกรรม</span>
				{#if !loading && certificates.length > 0}
					<Badge variant="outline">{certificates.length.toLocaleString('th-TH')} ใบ</Badge>
				{/if}
			</div>
			<div class="flex flex-wrap gap-2">
				<Button variant="outline" disabled={loading} onclick={() => loadCertificates(campaignId)}>
					<RefreshCw class="size-4" /> โหลดใหม่
				</Button>
				{#if canDownload}
					<Button
						disabled={loading || selectedCertificateIds.length === 0}
						onclick={() => (batchOpen = true)}
					>
						<Download class="size-4" /> ดาวน์โหลดที่เลือก
						{selectedCertificateIds.length.toLocaleString('th-TH')} ใบ
					</Button>
				{/if}
			</div>
		</div>
	{/if}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูใบที่ออกแล้ว"
			description="หน้านี้เปิดได้เมื่อมีสิทธิ์อ่านเกียรติบัตรของหน่วยงานหรือระดับโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดใบที่ออกแล้วไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadCertificates(campaignId)}
		/>
	{:else}
		<div class="space-y-4">
			<section class="overflow-hidden rounded-xl border bg-card shadow-sm">
				<div class="grid gap-px bg-border sm:grid-cols-3">
					<div class="relative overflow-hidden bg-card p-5">
						<div class="absolute inset-y-0 left-0 w-1 bg-emerald-500"></div>
						<p class="text-xs font-medium text-muted-foreground">ใช้งานได้</p>
						<p class="mt-1 text-3xl font-semibold tabular-nums text-emerald-800">
							{issuedCount.toLocaleString('th-TH')}
						</p>
						<p class="mt-1 text-xs text-muted-foreground">ดาวน์โหลดและตรวจสอบได้</p>
					</div>
					<div class="relative overflow-hidden bg-card p-5">
						<div class="absolute inset-y-0 left-0 w-1 bg-red-400"></div>
						<p class="text-xs font-medium text-muted-foreground">เพิกถอนแล้ว</p>
						<p class="mt-1 text-3xl font-semibold tabular-nums text-red-800">
							{revokedCount.toLocaleString('th-TH')}
						</p>
						<p class="mt-1 text-xs text-muted-foreground">คงเลขไว้และดาวน์โหลดไม่ได้</p>
					</div>
					<div class="relative overflow-hidden bg-card p-5">
						<div class="absolute inset-y-0 left-0 w-1 bg-blue-400"></div>
						<p class="text-xs font-medium text-muted-foreground">เชื่อมบัญชีภายใน</p>
						<p class="mt-1 text-3xl font-semibold tabular-nums text-blue-800">
							{linkedRecipientCount.toLocaleString('th-TH')}
						</p>
						<p class="mt-1 text-xs text-muted-foreground">นักเรียนและบุคลากร</p>
					</div>
				</div>
			</section>

			<div
				class="grid gap-3 rounded-xl border bg-card p-3 lg:grid-cols-[minmax(16rem,1fr)_12rem_16rem]"
			>
				<label class="relative block">
					<span class="sr-only">ค้นหาเลขหรือชื่อผู้รับ</span>
					<Search
						class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
					/>
					<Input bind:value={search} class="pl-9" placeholder="ค้นหาเลขเกียรติบัตรหรือชื่อผู้รับ" />
				</label>

				<Select.Root type="single" bind:value={statusFilter}>
					<Select.Trigger class="w-full" aria-label="กรองตามสถานะ">
						{statusFilter === 'all'
							? 'ทุกสถานะ'
							: statusFilter === 'issued'
								? 'ใช้งานได้'
								: 'เพิกถอนแล้ว'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">ทุกสถานะ</Select.Item>
						<Select.Item value="issued">ใช้งานได้</Select.Item>
						<Select.Item value="revoked">เพิกถอนแล้ว</Select.Item>
					</Select.Content>
				</Select.Root>

				<Select.Root type="single" bind:value={templateFilter}>
					<Select.Trigger class="w-full" aria-label="กรองตามแบบเกียรติบัตร">
						{templateFilter === 'all'
							? 'ทุกแบบเกียรติบัตร'
							: (templateOptions.find(([id]) => id === templateFilter)?.[1] ?? 'แบบเกียรติบัตร')}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">ทุกแบบเกียรติบัตร</Select.Item>
						{#each templateOptions as [id, name] (id)}
							<Select.Item value={id}>{name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			{#if filteredCertificates.length === 0}
				<PageState
					title={certificates.length === 0 ? 'ยังไม่มีเกียรติบัตรที่ออกเลขแล้ว' : 'ไม่พบใบที่ค้นหา'}
					description={certificates.length === 0
						? 'เมื่อคำขอได้รับการยืนยัน ใบที่ออกเลขแล้วจะปรากฏในทะเบียนนี้'
						: 'ลองเปลี่ยนเลข ชื่อผู้รับ สถานะ หรือแบบเกียรติบัตร'}
				/>
			{:else}
				<div class="overflow-x-auto rounded-xl border bg-card shadow-sm">
					<Table.Root class="min-w-[1180px]">
						<Table.Header>
							<Table.Row class="bg-muted/40 hover:bg-muted/40">
								<Table.Head class="w-12">
									<input
										type="checkbox"
										class="size-4 rounded border-input accent-primary"
										aria-label="เลือกเกียรติบัตรที่ดาวน์โหลดได้ทั้งหมด"
										checked={allFilteredSelected}
										disabled={selectableFilteredCertificates.length === 0}
										onchange={(event) => toggleFiltered(event.currentTarget.checked)}
									/>
								</Table.Head>
								<Table.Head class="w-52">เลขเกียรติบัตร</Table.Head>
								<Table.Head class="w-56">ผู้รับ</Table.Head>
								<Table.Head class="w-28">ประเภท</Table.Head>
								<Table.Head class="w-52">กิจกรรม / รางวัล</Table.Head>
								<Table.Head class="w-48">แบบ</Table.Head>
								<Table.Head class="w-32">วันที่ออก</Table.Head>
								<Table.Head class="w-36">สถานะ</Table.Head>
								<Table.Head class="w-56 text-right">จัดการ</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each filteredCertificates as certificate (certificate.id)}
								<Table.Row class={certificate.status === 'revoked' ? 'opacity-70' : undefined}>
									<Table.Cell>
										{#if canSelect(certificate)}
											<input
												type="checkbox"
												class="size-4 rounded border-input accent-primary"
												aria-label={`เลือก ${certificate.certificateNumber}`}
												checked={selectedCertificateIds.includes(certificate.id)}
												onchange={(event) =>
													toggleCertificate(certificate, event.currentTarget.checked)}
											/>
										{/if}
									</Table.Cell>
									<Table.Cell>
										<p class="font-mono font-semibold tabular-nums text-foreground">
											{certificate.certificateNumber}
										</p>
										{#if certificate.replacementForCertificateId}
											<Badge variant="secondary" class="mt-1">ออกทดแทนใบเดิม</Badge>
										{/if}
									</Table.Cell>
									<Table.Cell class="font-medium">{displayName(certificate)}</Table.Cell>
									<Table.Cell>{recipientLabels[certificate.recipientType]}</Table.Cell>
									<Table.Cell>
										<p>{certificate.activityItem ?? '-'}</p>
										{#if certificate.awardOrRole}
											<p class="mt-1 text-xs text-muted-foreground">{certificate.awardOrRole}</p>
										{/if}
									</Table.Cell>
									<Table.Cell>
										<span class="inline-flex items-center gap-1.5">
											<FileBadge2 class="size-4 text-muted-foreground" />
											{certificate.templateName}
										</span>
									</Table.Cell>
									<Table.Cell>{formatDate(certificate.issueDate)}</Table.Cell>
									<Table.Cell>
										{#if certificate.status === 'issued'}
											<Badge
												variant="outline"
												class="border-emerald-200 bg-emerald-50 text-emerald-800"
											>
												<Award class="size-3.5" /> ใช้งานได้
											</Badge>
										{:else}
											<Badge variant="outline" class="border-red-200 bg-red-50 text-red-800">
												<ShieldAlert class="size-3.5" /> เพิกถอนแล้ว
											</Badge>
											{#if certificate.replacementCandidateId}
												<a
													href={resolve(
														`/staff/certificates/${campaignId}/recipients#candidate-${certificate.replacementCandidateId}` as '/staff/certificates/[campaignId]/recipients'
													)}
													class="mt-2 block text-xs font-medium text-primary underline underline-offset-4"
												>
													ไปแก้รายการทดแทน
												</a>
											{/if}
										{/if}
									</Table.Cell>
									<Table.Cell>
										<div class="flex justify-end gap-2">
											<CertificateDownloadButton {certificate} {canDownload} />
											{#if canRevoke && certificate.status === 'issued' && certificate.capabilities.canRevoke}
												<Button
													size="sm"
													variant="destructive"
													onclick={() => (revokeTarget = certificate)}
													aria-label={`เพิกถอน ${certificate.certificateNumber}`}
												>
													<ShieldAlert class="size-4" /> เพิกถอน
												</Button>
											{/if}
										</div>
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			{/if}

			<div class="flex items-center gap-2 text-xs text-muted-foreground">
				<UsersRound class="size-4" />
				แสดง {filteredCertificates.length.toLocaleString('th-TH')} จาก
				{certificates.length.toLocaleString('th-TH')} ใบ
			</div>
		</div>
	{/if}
</div>

<CertificateBatchDownloadDialog
	open={batchOpen}
	{campaignId}
	{campaignName}
	{certificates}
	{selectedCertificateIds}
	onopenchange={(open) => (batchOpen = open)}
	ondownloaded={() => (selectedCertificateIds = [])}
/>

{#if revokeTarget}
	{#key revokeTarget.id}
		<CertificateRevokeDialog
			open={true}
			certificate={revokeTarget}
			{canRevoke}
			onopenchange={(open) => !open && (revokeTarget = null)}
			onrevoked={handleRevoked}
		/>
	{/key}
{/if}
