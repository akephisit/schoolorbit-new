<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createActivityVersion,
		createCatalogActivity,
		listActivityVersions,
		listCatalogActivities,
		publishActivityVersion,
		type ActivityVersion,
		type CatalogActivity
	} from '$lib/api/academic-core';
	import CatalogVersionHistory from '$lib/components/academic-core/CatalogVersionHistory.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Plus } from 'lucide-svelte';

	type VersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};
	let activities = $state<CatalogActivity[]>([]);
	let selected = $state<CatalogActivity | null>(null);
	let versions = $state<ActivityVersion[]>([]);
	let loading = $state(true);
	let errorMessage = $state('');
	let code = $state('');
	let activityType = $state('development');
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT
		)
	);

	async function loadWorkspace() {
		loading = true;
		errorMessage = '';
		try {
			activities = await listCatalogActivities();
			if (activities[0]) await selectActivity(activities[0]);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดทะเบียนกิจกรรมไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}
	async function selectActivity(activity: CatalogActivity) {
		selected = activity;
		versions = await listActivityVersions(activity.id);
	}
	async function addActivity(event: SubmitEvent) {
		event.preventDefault();
		const created = await createCatalogActivity({
			code,
			activityType,
			owningOrganizationUnitId: null
		});
		activities = [...activities, created];
		code = '';
		await selectActivity(created);
	}
	async function addVersion(draft: VersionDraft) {
		if (!selected) return;
		const created = await createActivityVersion(selected.id, {
			name: draft.name,
			hoursPerWeek: draft.exactValue,
			description: draft.secondaryName || null,
			effectiveFrom: draft.effectiveFrom,
			effectiveUntil: draft.effectiveUntil || null,
			gradeLevelIds: draft.gradeLevelIds,
			schedulingMode: draft.classification,
			termCode: null
		});
		versions = [...versions, created];
	}
	async function publish(id: string, rowVersion: number) {
		const updated = await publishActivityVersion(id, { rowVersion });
		versions = versions.map((item) => (item.id === id ? updated : item));
	}
	onMount(loadWorkspace);
</script>

<PageShell
	title="ทะเบียนกิจกรรม"
	description="กิจกรรมพัฒนาผู้เรียนอยู่ในทะเบียนและมีรุ่นข้อมูลเช่นเดียวกับรายวิชา"
>
	{#if loading}<PageSkeleton
			variant="cards"
			rows={4}
		/>{:else if errorMessage && activities.length === 0}<PageState
			variant="error"
			title="โหลดทะเบียนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={loadWorkspace}
		/>{:else}<div class="grid gap-5 xl:grid-cols-[260px_minmax(0,1fr)]">
			<aside class="space-y-3 rounded-xl border bg-card p-4">
				<h2 class="font-semibold">รหัสกิจกรรม</h2>
				{#each activities as activity (activity.id)}<button
						class:border-primary={selected?.id === activity.id}
						class="w-full rounded-lg border px-3 py-2 text-left text-sm hover:bg-muted"
						onclick={() => selectActivity(activity)}
						><span class="font-medium">{activity.code}</span><span
							class="block text-xs text-muted-foreground">{activity.activityType}</span
						></button
					>{/each}{#if canManage}<form class="space-y-2 border-t pt-3" onsubmit={addActivity}>
						<Label class="sr-only" for="new-activity-code">รหัสใหม่</Label><Input
							id="new-activity-code"
							bind:value={code}
							placeholder="รหัสกิจกรรม"
							required
						/><Label class="sr-only" for="new-activity-type">ประเภท</Label><Input
							id="new-activity-type"
							bind:value={activityType}
							placeholder="ประเภท"
							required
						/><Button class="w-full" type="submit"><Plus class="size-4" /> เพิ่มกิจกรรม</Button>
					</form>{/if}
			</aside>
			{#if selected}<CatalogVersionHistory
					kind="activity"
					code={selected.code}
					items={versions.map((item) => ({
						id: item.id,
						versionNo: item.versionNo,
						name: item.name,
						secondaryName: item.description,
						exactValue: `${item.hoursPerWeek} ชม./สัปดาห์`,
						effectiveFrom: item.effectiveFrom,
						effectiveUntil: item.effectiveUntil,
						status: item.status,
						rowVersion: item.rowVersion
					}))}
					{canManage}
					onCreate={addVersion}
					onPublish={publish}
				/>{:else}<div
					class="rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
				>
					เลือกกิจกรรมเพื่อดูประวัติรุ่น
				</div>{/if}
		</div>
		{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}{/if}
</PageShell>
