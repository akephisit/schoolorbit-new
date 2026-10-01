<script lang="ts">
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { resolve } from '$app/paths';
	import { consentApi, type UserConsentStatus, type ConsentRecord } from '$lib/api/consent';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import * as Card from '$lib/components/ui/card';
	import * as Alert from '$lib/components/ui/alert';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import StatusBadge from '$lib/components/consent/StatusBadge.svelte';
	import { LoaderCircle, CheckCircle2, XCircle, Clock, Shield, Info } from '@lucide/svelte';
	import { formatDistanceToNow } from 'date-fns';
	import { th } from 'date-fns/locale';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${data.requestKey}`);
	const allowed = $derived($authStore.isAuthenticated);
	let status = $state.raw<UserConsentStatus | null>(null);
	let loading = $state(true),
		withdrawing = $state<string | null>(null),
		error = $state<string | null>(null);
	const activeConsents = $derived(
		status?.consents.filter((c) => c.consent_status === 'granted' && !c.is_expired) ?? []
	);
	const withdrawnConsents = $derived(
		status?.consents.filter((c) => c.consent_status === 'withdrawn') ?? []
	);
	const statusRequest = new LatestRequest();
	let owner = '',
		disposed = false;
	let consumedSource: typeof data.consent | null = null;
	$effect.pre(() => {
		const key = ownerKey,
			source = data.consent,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;
				statusRequest.abort();
				status = null;
				loading = canRead;
				withdrawing = null;
				error = null;
			}
			if (!canRead || consumedSource === source) return;
			consumedSource = source;
			const request = statusRequest.begin();
			loading = true;
			void source.then((result) => applyStatus(result, request.revision, key));
		});
	});
	onDestroy(() => {
		disposed = true;
		statusRequest.abort();
	});
	function current(key: string) {
		return !disposed && allowed && key === ownerKey;
	}
	function applyStatus(result: Awaited<typeof data.consent>, revision: number, key: string) {
		if (!current(key) || !statusRequest.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			error = result.error;
			return;
		}
		if (result.data.ownerKey !== key) return;
		status = result.data.status;
		error = null;
	}
	async function loadConsentStatus() {
		if (!allowed || disposed) return;
		const key = ownerKey,
			request = statusRequest.begin();
		loading = true;
		error = null;
		const result = await captureRouteLoad(
			consentApi
				.getMyConsentStatus({ signal: request.signal })
				.then((status) => ({ ownerKey: key, status })),
			'เกิดข้อผิดพลาดในการโหลดข้อมูล'
		);
		applyStatus(result, request.revision, key);
	}
	async function handleWithdraw(consent: ConsentRecord) {
		if (
			!allowed ||
			disposed ||
			withdrawing !== null ||
			!status?.consents.some((row) => row.id === consent.id && row.consent_status === 'granted')
		)
			return;
		if (consent.is_required) {
			alert('ไม่สามารถถอนความยินยอมที่จำเป็นได้');
			return;
		}
		const key = ownerKey;
		if (
			!confirm(
				`ต้องการถอนความยินยอม "${consent.consent_type_name || consent.consent_type}" หรือไม่?`
			) ||
			!current(key)
		)
			return;
		statusRequest.abort();
		loading = false;
		withdrawing = consent.id;
		error = null;
		try {
			await consentApi.withdrawConsent(consent.id);
			if (!current(key)) return;
			await loadConsentStatus();
		} catch (err) {
			if (current(key)) error = err instanceof Error ? err.message : 'ไม่สามารถถอนความยินยอมได้';
		} finally {
			if (current(key)) withdrawing = null;
		}
	}

	function getStatusBadge(consent: ConsentRecord) {
		if (consent.is_expired) {
			return {
				variant: 'secondary' as const,
				icon: Clock,
				label: 'หมดอายุ'
			};
		}

		switch (consent.consent_status) {
			case 'granted':
				return {
					variant: 'default' as const,
					icon: CheckCircle2,
					label: 'อนุญาต'
				};
			case 'withdrawn':
				return {
					variant: 'destructive' as const,
					icon: XCircle,
					label: 'ถอนคืน'
				};
			case 'denied':
				return {
					variant: 'destructive' as const,
					icon: XCircle,
					label: 'ปฏิเสธ'
				};
			default:
				return {
					variant: 'outline' as const,
					icon: Clock,
					label: 'รอดำเนินการ'
				};
		}
	}

	function formatDate(dateString: string | null): string {
		if (!dateString) return '-';
		try {
			return formatDistanceToNow(new Date(dateString), {
				addSuffix: true,
				locale: th
			});
		} catch {
			return dateString;
		}
	}
</script>

<PageShell
	title="จัดการความยินยอม"
	description="จัดการความยินยอมการเก็บและใช้ข้อมูลส่วนบุคคลของคุณ ตาม พ.ร.บ. คุ้มครองข้อมูลส่วนบุคคล พ.ศ. 2562"
>
	<Button variant="outline" disabled={loading || withdrawing !== null} onclick={loadConsentStatus}
		>โหลดข้อมูลใหม่</Button
	>
	<div data-testid="consent-region" aria-busy={loading}>
		{#if loading && status}<p
				role="status"
				aria-label="กำลังอัปเดตความยินยอม"
				class="text-muted-foreground text-sm"
			>
				กำลังอัปเดตความยินยอม…
			</p>{/if}
		{#if loading && !status}
			<div role="status" aria-label="กำลังโหลดความยินยอม"><PageSkeleton variant="detail" /></div>
		{:else if error && !status}
			<PageState
				variant="error"
				title="โหลดข้อมูลความยินยอมไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadConsentStatus}
			/>
		{:else if status}
			{#if error}
				<PageState
					variant="error"
					title="ดำเนินการไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadConsentStatus}
				/>
			{/if}
			<!-- Compliance Status -->
			<Card.Root class={status.is_compliant ? 'border-green-500' : 'border-yellow-500'}>
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<Shield class={status.is_compliant ? 'text-green-500' : 'text-yellow-500'} />
						สถานะความสมบูรณ์
					</Card.Title>
				</Card.Header>
				<Card.Content class="space-y-4">
					<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
						<div>
							<p class="text-sm text-muted-foreground">ความยินยอมทั้งหมด</p>
							<p class="text-2xl font-bold">{status.consents.length}</p>
						</div>
						<div>
							<p class="text-sm text-muted-foreground">ความยินยอมที่จำเป็น</p>
							<p class="text-2xl font-bold">
								{status.granted_required} / {status.total_required}
							</p>
						</div>
						<div>
							<p class="text-sm text-muted-foreground">สถานะ</p>
							<p
								class="text-2xl font-bold {status.is_compliant
									? 'text-green-500'
									: 'text-yellow-500'}"
							>
								{status.is_compliant ? '✓ สมบูรณ์' : '⚠ ไม่สมบูรณ์'}
							</p>
						</div>
					</div>

					{#if !status.is_compliant && status.missing_required_consents.length > 0}
						<Alert.Root variant="default" class="border-yellow-500">
							<Info class="h-4 w-4" />
							<Alert.Title>ความยินยอมที่ขาดหาย</Alert.Title>
							<Alert.Description>
								คุณยังไม่ได้ให้ความยินยอมในรายการต่อไปนี้:
								<ul class="list-disc list-inside mt-2">
									{#each status.missing_required_consents as code (code)}
										<li class="text-sm">{code}</li>
									{/each}
								</ul>
							</Alert.Description>
						</Alert.Root>
					{/if}
				</Card.Content>
			</Card.Root>

			<!-- Active Consents -->
			{#if activeConsents.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title>ความยินยอมที่ใช้งานอยู่</Card.Title>
						<Card.Description>ความยินยอมที่คุณให้ไว้และยังมีผลบังคับใช้</Card.Description>
					</Card.Header>
					<Card.Content class="space-y-4">
						{#each activeConsents as consent (consent.id)}
							<div class="rounded-lg border p-4 space-y-3">
								<!-- Header -->
								<div class="flex items-start justify-between gap-4">
									<div class="flex-1">
										<div class="flex items-center gap-2 mb-1">
											<h3 class="font-medium">
												{consent.consent_type_name || consent.consent_type}
											</h3>
											{#if consent.is_required}
												<Badge variant="destructive" class="text-xs">จำเป็น</Badge>
											{/if}
										</div>
										<p class="text-sm text-muted-foreground">
											{consent.purpose}
										</p>
									</div>
									<StatusBadge {...getStatusBadge(consent)} />
								</div>

								<!-- Metadata -->
								<div class="grid grid-cols-1 md:grid-cols-2 gap-2 text-xs text-muted-foreground">
									{#if consent.granted_at}
										<div>
											ให้ความยินยอมเมื่อ: {formatDate(consent.granted_at)}
										</div>
									{/if}
									{#if consent.expires_at && !consent.is_expired}
										<div>
											หมดอายุ: {formatDate(consent.expires_at)}
										</div>
									{/if}
									{#if consent.is_minor_consent && consent.parent_guardian_name}
										<div class="col-span-full">
											ผู้ปกครอง: {consent.parent_guardian_name}
										</div>
									{/if}
								</div>

								<!-- Action -->
								{#if !consent.is_required}
									<div class="pt-2 border-t">
										<Button
											variant="destructive"
											size="sm"
											onclick={() => handleWithdraw(consent)}
											disabled={withdrawing !== null}
										>
											{#if withdrawing === consent.id}
												<LoaderCircle class="h-3 w-3 animate-spin mr-2" />
												กำลังถอน...
											{:else}
												<XCircle class="h-3 w-3 mr-2" />
												ถอนความยินยอม
											{/if}
										</Button>
									</div>
								{/if}
							</div>
						{/each}
					</Card.Content>
				</Card.Root>
			{/if}

			<!-- Withdrawn Consents -->
			{#if withdrawnConsents.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title>ความยินยอมที่ถอนแล้ว</Card.Title>
						<Card.Description>ความยินยอมที่คุณได้ถอนคืนไปแล้ว</Card.Description>
					</Card.Header>
					<Card.Content class="space-y-4">
						{#each withdrawnConsents as consent (consent.id)}
							<div class="rounded-lg border p-4 space-y-2 opacity-60">
								<div class="flex items-start justify-between gap-4">
									<div class="flex-1">
										<h3 class="font-medium">
											{consent.consent_type_name || consent.consent_type}
										</h3>
										<p class="text-sm text-muted-foreground">
											ถอนเมื่อ: {formatDate(consent.withdrawn_at)}
										</p>
									</div>
									<Badge variant="destructive">
										<XCircle class="h-3 w-3 mr-1" />
										ถอนคืน
									</Badge>
								</div>
							</div>
						{/each}
					</Card.Content>
				</Card.Root>
			{/if}

			<!-- Privacy Policy Link -->
			<Card.Root class="gap-0 py-0">
				<Card.Content class="p-4">
					<p class="text-sm text-muted-foreground text-center">
						อ่านเพิ่มเติมที่
						<a href={resolve('/privacy-policy')} class="text-primary underline hover:no-underline">
							นโยบายความเป็นส่วนตัว
						</a>
						หรือติดต่อเจ้าหน้าที่คุ้มครองข้อมูลส่วนบุคคล (DPO)
					</p>
				</Card.Content>
			</Card.Root>
		{:else}
			<PageState
				title="ไม่พบข้อมูลความยินยอม"
				description="ยังไม่มีสถานะความยินยอมสำหรับบัญชีนี้"
				actionLabel="โหลดอีกครั้ง"
				onaction={loadConsentStatus}
			/>
		{/if}
	</div>
</PageShell>
