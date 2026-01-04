<script lang="ts">
	import { listFeatures, toggleFeature, type FeatureToggle } from '$lib/api/feature-toggles';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import { Switch } from '$lib/components/ui/switch';
	import { Badge } from '$lib/components/ui/badge';
	import { LoaderCircle, Power, Shield } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let features = $state<FeatureToggle[]>([]);
	let loading = $state(true);
	let toggleLoading = $state<Record<string, boolean>>({});

	// Load features on mount
	$effect(() => {
		loadFeatures();
	});

	async function loadFeatures() {
		try {
			loading = true;
			features = await listFeatures();
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้';
			toast.error(message);
		} finally {
			loading = false;
		}
	}

	async function handleToggle(feature: FeatureToggle) {
		try {
			toggleLoading[feature.id] = true;
			const updated = await toggleFeature(feature.id);

			// Update local state
			features = features.map((f) => (f.id === feature.id ? updated : f));

			const status = updated.is_enabled ? 'เปิดใช้งาน' : 'ปิดใช้งาน';
			toast.success(`${status} ${feature.name} สำเร็จ`);
		} catch (error) {
			const message = error instanceof Error ? error.message : 'ไม่สามารถเปลี่ยนสถานะได้';
			toast.error(message);
		} finally {
			toggleLoading[feature.id] = false;
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

<svelte:head>
	<title>จัดการระบบงาน - Feature Toggles</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold">จัดการระบบงาน</h1>
			<p class="text-muted-foreground mt-1">เปิด/ปิดการทำงานของระบบย่อยต่างๆ</p>
		</div>
		<Button onclick={loadFeatures} variant="outline" disabled={loading}>
			{#if loading}
				<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
			{/if}
			รีเฟรช
		</Button>
	</div>

	<!-- Loading State -->
	{#if loading}
		<div class="flex justify-center items-center py-20">
			<LoaderCircle class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else if features.length === 0}
		<!-- Empty State -->
		<Card class="p-12">
			<div class="text-center text-muted-foreground">
				<Shield class="h-16 w-16 mx-auto mb-4 opacity-20" />
				<p class="text-lg">ไม่พบระบบงานที่คุณสามารถจัดการได้</p>
				<p class="text-sm mt-2">กรุณาตรวจสอบสิทธิ์การเข้าถึงของคุณ</p>
			</div>
		</Card>
	{:else}
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
											checked={feature.is_enabled}
											onCheckedChange={() => handleToggle(feature)}
											disabled={toggleLoading[feature.id]}
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
</div>

<style>
	:global(body) {
		background: hsl(var(--background));
	}
</style>
