<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { ArrowUpRight, UsersRound } from '@lucide/svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { getPersonnelOverview, type PersonnelOverview } from '#lib/api/personnel.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import RankMilestoneOverview from '#lib/components/staff/RankMilestoneOverview.svelte';
	import PersonnelBarChart from '#lib/components/staff/PersonnelBarChart.svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can, userPermissions } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let overview = $state<PersonnelOverview | null>(null),
		loading = $state(true),
		error = $state('');
	let activeStatus = '';
	const request = new LatestRequest();
	const identityKey = $derived.by(() => {
		void $authStore.user;
		void $userPermissions;
		return appIdentityKey();
	});
	let sourceOwner = '';
	let previousSource: typeof data.overview | undefined;
	const allowed = $derived(
		$can.hasAny(
			PERMISSIONS.STAFF_PROFILE_READ_OWN,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
			PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
		)
	);
	$effect.pre(() => {
		const status = `${identityKey}:${data.status}`,
			source = data.overview;
		untrack(() => {
			if (status !== activeStatus) {
				activeStatus = status;
				overview = null;
			}
			if (!identityKey || (source === previousSource && sourceOwner !== identityKey)) {
				request.abort();
				loading = Boolean(identityKey);
				return;
			}
			previousSource = source;
			sourceOwner = identityKey;
			const ticket = request.begin();
			loading = true;
			error = '';
			void source.then((result) => apply(result, ticket.revision));
		});
		return () => request.abort();
	});
	$effect(() => {
		if (!allowed) {
			request.abort();
			overview = null;
			loading = false;
		}
	});
	onDestroy(() => request.abort());
	function apply(result: Awaited<typeof data.overview>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) overview = result.data;
		else error = result.error;
	}
	async function refresh() {
		if (!allowed) return;
		const ticket = request.begin();
		loading = true;
		error = '';
		apply(
			await captureRouteLoad(
				getPersonnelOverview({ status: data.status }, { signal: ticket.signal }),
				'โหลดภาพรวมงานบุคคลไม่สำเร็จ'
			),
			ticket.revision
		);
	}
</script>

<PageShell
	title="ภาพรวมงานบุคคล"
	description="สรุปบุคลากรที่เปิดใช้งานและข้อมูลตามสังกัดปัจจุบันในขอบเขตที่คุณมีสิทธิ์ดู"
>
	{#snippet actions()}
		<Button href="/staff/manage?status=active" data-sveltekit-preload-data="tap">
			<UsersRound class="size-4" aria-hidden="true" />
			ดูรายชื่อบุคลากร
		</Button>
	{/snippet}
	<section data-testid="personnel-overview" aria-busy={loading} class="space-y-5">
		{#if error}<PageState
				variant="error"
				title={error}
				actionLabel="ลองอีกครั้ง"
				onaction={refresh}
			/>{/if}
		{#if !allowed}<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูภาพรวมงานบุคคล"
			/>{:else if !overview && loading}
			<div role="status" aria-label="กำลังโหลดภาพรวมงานบุคคล" class="space-y-5">
				<Card.Root>
					<Card.Content class="flex flex-wrap items-center justify-between gap-4">
						<div class="flex items-center gap-4">
							<Skeleton class="size-11 rounded-xl" />
							<div class="space-y-2">
								<Skeleton class="h-5 w-32" />
								<Skeleton class="h-9 w-20" />
							</div>
						</div>
						<Skeleton class="h-5 w-48" />
						<Skeleton class="h-8 w-28" />
					</Card.Content>
				</Card.Root>
				<div class="columns-1 gap-5 lg:columns-2">
					{#each Array.from({ length: 4 }) as _, index (index)}
						<Card.Root class="mb-5 break-inside-avoid">
							<Card.Content class="space-y-4">
								<Skeleton class="h-5 w-48" />
								<Skeleton class="h-12 w-full" />
								<Skeleton class="h-12 w-full" />
							</Card.Content>
						</Card.Root>
					{/each}
				</div>
			</div>
		{:else if overview}
			<Card.Root
				class="border-primary/15 bg-gradient-to-br from-primary/5 via-card to-card"
				role="region"
				aria-label="บุคลากรทั้งหมด"
			>
				<Card.Content class="flex flex-wrap items-center justify-between gap-4">
					<div class="flex items-center gap-4">
						<div
							class="flex size-11 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
							aria-hidden="true"
						>
							<UsersRound class="size-5" />
						</div>
						<dl class="space-y-1">
							<dt class="text-sm font-medium text-muted-foreground">บุคลากรทั้งหมด</dt>
							<dd
								class="flex items-baseline gap-2 text-3xl font-semibold tracking-tight tabular-nums"
							>
								{overview.total.toLocaleString('th-TH')}
								<span class="text-sm font-normal text-muted-foreground">คน</span>
							</dd>
						</dl>
					</div>
					<p class="text-sm text-muted-foreground">เฉพาะบุคลากรที่เปิดใช้งาน</p>
					<Button
						variant="secondary"
						size="sm"
						href="/staff/manage?status=active"
						aria-label="ดูรายชื่อบุคลากรทั้งหมด"
						data-sveltekit-preload-data="tap"
					>
						ดูรายชื่อ
						<ArrowUpRight class="size-4" aria-hidden="true" />
					</Button>
				</Card.Content>
			</Card.Root>
			<div class="columns-1 gap-5 lg:columns-2">
				<PersonnelBarChart
					title="บุคลากรตามกลุ่มสาระ"
					dimension="subject_group"
					buckets={overview.subjectGroups}
					status={data.status}
					note="หนึ่งคนอาจอยู่หลายกลุ่มสาระ ผลรวมกราฟจึงอาจมากกว่าจำนวนบุคลากร ใช้สังกัดปัจจุบันในระบบ"
				/>
				<PersonnelBarChart
					title="บุคลากรตามตำแหน่งงาน"
					dimension="job_position"
					buckets={overview.jobPositions}
					status={data.status}
				/>
				<PersonnelBarChart
					title="บุคลากรตามวิทยฐานะ"
					dimension="academic_rank"
					buckets={overview.academicRanks}
					status={data.status}
				/>
				<PersonnelBarChart
					title="บุคลากรตามวุฒิการศึกษาสูงสุด"
					note="ใช้ระดับวุฒิสูงสุดที่จัดลำดับได้ หากมีเฉพาะวุฒิอื่นจะแยกเป็นกลุ่มอื่น ๆ และแยกข้อมูลที่ยังไม่ระบุระดับ"
					dimension="education_level"
					buckets={overview.educationLevels}
					status={data.status}
				/>
			</div>
			<p class="text-xs text-muted-foreground">
				ข้อมูล ณ {new Date(overview.asOf).toLocaleString('th-TH')} · กดกราฟเพื่อดูรายชื่อ · ช่องที่ยังไม่ระบุแสดงแยกจากค่าที่ระบุแล้ว
			</p>
		{/if}
	</section>
	{#if allowed}<RankMilestoneOverview
			initial={data.rankMilestones}
			status={data.status}
			ownerKey={identityKey}
		/>{/if}
</PageShell>
