<script lang="ts">
	import { onMount } from 'svelte';
	import { consentApi, type UserConsentStatus, type ConsentRecord } from '$lib/api/consent';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Alert from '$lib/components/ui/alert';
	import { Badge } from '$lib/components/ui/badge';
	import StatusBadge from '$lib/components/consent/StatusBadge.svelte';
	import {
		LoaderCircle,
		AlertCircle,
		CheckCircle2,
		XCircle,
		Clock,
		Shield,
		Info
	} from 'lucide-svelte';
	import { formatDistanceToNow } from 'date-fns';
	import { th } from 'date-fns/locale';

	// State
	let status = $state<UserConsentStatus | null>(null);
	let loading = $state(true);
	let withdrawing = $state<string | null>(null);
	let error = $state<string | null>(null);

	// Computed
	const activeConsents = $derived(
		status?.consents.filter((c) => c.consent_status === 'granted' && !c.is_expired) || []
	);
	const withdrawnConsents = $derived(
		status?.consents.filter((c) => c.consent_status === 'withdrawn') || []
	);
	const expiredConsents = $derived(status?.consents.filter((c) => c.is_expired) || []);

	// Load consent status
	onMount(async () => {
		await loadConsentStatus();
	});

	async function loadConsentStatus() {
		try {
			loading = true;
			error = null;
			status = await consentApi.getMyConsentStatus();
		} catch (err) {
			error = err instanceof Error ? err.message : 'เกิดข้อผิดพลาดในการโหลดข้อมูล';
		} finally {
			loading = false;
		}
	}

	async function handleWithdraw(consent: ConsentRecord) {
		if (consent.is_required) {
			alert('ไม่สามารถถอนความยินยอมที่จำเป็นได้');
			return;
		}

		if (
			!confirm(
				`ต้องการถอนความยินยอม "${consent.consent_type_name || consent.consent_type}" หรือไม่?`
			)
		) {
			return;
		}

		try {
			withdrawing = consent.id;
			error = null;
			await consentApi.withdrawConsent(consent.id);

			// Reload status
			await loadConsentStatus();
		} catch (err) {
			error = err instanceof Error ? err.message : 'ไม่สามารถถอนความยินยอมได้';
		} finally {
			withdrawing = null;
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

<svelte:head>
	<title>จัดการความยินยอม - PDPA</title>
</svelte:head>

<div class="container max-w-5xl py-8 space-y-6">
	<!-- Header -->
	<div class="space-y-2">
		<h1 class="text-3xl font-bold">จัดการความยินยอม</h1>
		<p class="text-muted-foreground">
			จัดการความยินยอมการเก็บและใช้ข้อมูลส่วนบุคคลของคุณ ตาม พ.ร.บ. คุ้มครองข้อมูลส่วนบุคคล พ.ศ.
			2562
		</p>
	</div>

	<!-- Error Alert -->
	{#if error}
		<Alert.Root variant="destructive">
			<AlertCircle class="h-4 w-4" />
			<Alert.Title>เกิดข้อผิดพลาด</Alert.Title>
			<Alert.Description>{error}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if loading}
		<!-- Loading State -->
		<Card.Root>
			<Card.Content class="flex items-center justify-center p-12">
				<LoaderCircle class="h-8 w-8 animate-spin text-muted-foreground" />
			</Card.Content>
		</Card.Root>
	{:else if status}
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
								{#each status.missing_required_consents as code}
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
					{#each activeConsents as consent}
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
										disabled={withdrawing === consent.id}
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
					{#each withdrawnConsents as consent}
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
		<Card.Root>
			<Card.Content class="p-4">
				<p class="text-sm text-muted-foreground text-center">
					อ่านเพิ่มเติมที่
					<a href="/privacy-policy" class="text-primary underline hover:no-underline">
						นโยบายความเป็นส่วนตัว
					</a>
					หรือติดต่อเจ้าหน้าที่คุ้มครองข้อมูลส่วนบุคคล (DPO)
				</p>
			</Card.Content>
		</Card.Root>
	{/if}
</div>
