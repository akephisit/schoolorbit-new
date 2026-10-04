<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	let { data }: PageProps = $props();
	const source = $derived(data.features);
	const request = new LatestRequest();
	let owner = '',
		epoch = 0,
		disposed = false;
	let loaded = $state(false),
		loadError = $state('');
	import { listFeatures, toggleFeature, type FeatureToggle } from '#lib/api/feature-toggles.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Card } from '#lib/components/ui/card/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { LoaderCircle, Power } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let features = $state<FeatureToggle[]>([]);
	let loading = $state(true);
	let toggleLoading = $state<Record<string, boolean>>({});

	const canReadFeatures = $derived($can.has(PERMISSIONS.FEATURES_READ_ALL));
	const canUpdateFeatures = $derived($can.has(PERMISSIONS.FEATURES_UPDATE_ALL));

	function applyFeatures(result: Awaited<typeof data.features>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			features = result.data ?? [];
			loaded = true;
		} else loadError = result.error;
	}
	$effect.pre(() => {
		const operation = source,
			identity = `${$authStore.user?.id ?? ''}|${canReadFeatures}`;
		untrack(() => {
			if (owner !== identity) {
				owner = identity;
				epoch++;
				features = [];
				loaded = false;
				toggleLoading = {};
			}
			const ticket = request.begin();
			loading = true;
			loadError = '';
			void operation.then((result) => applyFeatures(result, ticket.revision));
		});
		return () => request.abort();
	});
	$effect.pre(() => {
		const allowed = canUpdateFeatures;
		untrack(() => {
			void allowed;
			epoch++;
			toggleLoading = {};
		});
	});
	onDestroy(() => {
		disposed = true;
		epoch++;
		request.abort();
	});
	async function loadFeatures() {
		if (!canReadFeatures) return;
		const ticket = request.begin();
		loading = true;
		loadError = '';
		applyFeatures(
			await captureRouteLoad(listFeatures({ signal: ticket.signal }), 'โหลดระบบงานไม่สำเร็จ'),
			ticket.revision
		);
	}

	async function handleToggle(feature: FeatureToggle) {
		if (disposed || toggleLoading[feature.id]) return;
		if (!canUpdateFeatures) {
			toast.error('ไม่มีสิทธิ์เปลี่ยนสถานะระบบงาน');
			return;
		}

		const mutationEpoch = epoch;
		const current = () => !disposed && mutationEpoch === epoch && canUpdateFeatures;
		request.abort();
		loading = false;
		try {
			toggleLoading[feature.id] = true;
			const updated = await toggleFeature(feature.id);
			if (!current()) return;
			request.abort();
			loading = false;

			// Update local state
			features = features.map((f) => (f.id === feature.id ? updated : f));

			const status = updated.is_enabled ? 'เปิดใช้งาน' : 'ปิดใช้งาน';
			toast.success(`${status} ${feature.name} สำเร็จ`);
		} catch (error) {
			if (!current()) return;
			const message = error instanceof Error ? error.message : 'ไม่สามารถเปลี่ยนสถานะได้';
			toast.error(message);
		} finally {
			if (current()) toggleLoading[feature.id] = false;
		}
	}

	// Group features by module (derived state)
	const featuresByModule = $derived(
		features.reduce(
			(acc, feature) => {
				const module = feature.module || 'อื่นๆ';
				if (!acc[module]) {
					acc[module] = [];
				}
				acc[module].push(feature);
				return acc;
			},
			{} as Record<string, FeatureToggle[]>
		)
	);
</script>

<PageShell title="จัดการระบบงาน" description="เปิด/ปิดการทำงานของระบบย่อยต่างๆ">
	{#snippet actions()}
		{#if canReadFeatures}
			<Button onclick={loadFeatures} variant="outline" disabled={loading}>
				{#if loading}
					<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				รีเฟรช
			</Button>
		{/if}
	{/snippet}

	<section data-testid="settings-features" aria-busy={loading}>
		{#if loading && loaded}<p role="status">กำลังอัปเดตระบบงาน</p>{/if}
		{#if loadError}<PageState
				variant="error"
				title="โหลดระบบงานไม่สำเร็จ"
				description={loadError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadFeatures}
			/>{/if}
		{#if !canReadFeatures}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูระบบงาน"
				description="บัญชีนี้เข้า module ระบบงานได้ แต่ยังไม่มีสิทธิ์อ่านรายการ feature toggles"
			/>
		{:else if loading && !loaded}
			<div role="status" aria-label="กำลังโหลดระบบงาน">
				<PageSkeleton variant="cards" rows={6} />
			</div>
		{:else if loaded && features.length === 0}
			<PageState
				title="ไม่พบระบบงานที่คุณสามารถจัดการได้"
				description="กรุณาตรวจสอบสิทธิ์การเข้าถึงของคุณ"
			/>
		{:else if loaded}
			<!-- Features by Module -->
			<div class="space-y-6">
				{#each Object.entries(featuresByModule) as [moduleName, moduleFeatures] (moduleName)}
					<div class="space-y-3">
						<!-- Module Header -->
						<div class="flex items-center gap-2">
							<h2 class="text-xl font-semibold capitalize">{moduleName}</h2>
							<Badge variant="secondary">{moduleFeatures.length} ระบบ</Badge>
						</div>

						<!-- Feature Cards -->
						<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
							{#each moduleFeatures as feature (feature.id)}
								<Card class="p-4">
									<div class="space-y-3">
										<!-- Feature Header -->
										<div class="flex items-start justify-between">
											<div class="flex-1">
												<h3 class="font-semibold">{feature.name}</h3>
												{#if feature.name_en}
													<p class="text-sm text-muted-foreground">{feature.name_en}</p>
												{/if}
												<p class="text-xs text-muted-foreground mt-1">
													<code class="bg-muted px-1 py-0.5 rounded">{feature.code}</code>
												</p>
											</div>
											<Badge variant={feature.is_enabled ? 'default' : 'secondary'}>
												{feature.is_enabled ? 'เปิด' : 'ปิด'}
											</Badge>
										</div>

										<!-- Toggle Control -->
										<div class="flex items-center justify-between pt-2 border-t">
											<div class="flex items-center gap-2">
												<Power class="h-4 w-4 text-muted-foreground" />
												<span class="text-sm text-muted-foreground">
													{feature.is_enabled ? 'ใช้งาน' : 'ปิดใช้งาน'}
												</span>
											</div>
											<Switch
												aria-label={`เปิด/ปิด ${feature.name}`}
												checked={feature.is_enabled}
												onCheckedChange={() => handleToggle(feature)}
												disabled={!canUpdateFeatures || toggleLoading[feature.id]}
											/>
										</div>
									</div>
								</Card>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</section>
</PageShell>

<style>
	:global(body) {
		background: hsl(var(--background));
	}
</style>
