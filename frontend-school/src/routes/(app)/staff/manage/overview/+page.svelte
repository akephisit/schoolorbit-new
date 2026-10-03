<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		ArrowUpRight,
		RefreshCw,
		UserRoundCheck,
		UserRoundMinus,
		UsersRound
	} from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { getPersonnelOverview, type PersonnelOverview } from '$lib/api/personnel';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import PersonnelBarChart from '$lib/components/staff/PersonnelBarChart.svelte';
	import PersonnelStatusChart from '$lib/components/staff/PersonnelStatusChart.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can, userPermissions } from '$lib/stores/permissions';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { STAFF_STATUS_OPTIONS, staffStatusLabel } from '$lib/forms/staff-status';
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
	description="สรุปจำนวนบุคลากรตามข้อมูลและสังกัดปัจจุบันในขอบเขตที่คุณมีสิทธิ์ดู"
>
	{#snippet actions()}
		<Button href="/staff/manage" data-sveltekit-preload-data="tap">
			<UsersRound class="size-4" aria-hidden="true" />
			ดูรายชื่อบุคลากร
		</Button>
		<Button variant="outline" onclick={refresh} disabled={loading}>
			<RefreshCw
				class={loading ? 'size-4 motion-safe:animate-spin' : 'size-4'}
				aria-hidden="true"
			/>
			รีเฟรชข้อมูล
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
			/>{:else if !overview && loading}<PageSkeleton variant="cards" rows={3} /><PageSkeleton
				variant="cards"
				rows={4}
			/>{:else if overview}
			<div class="grid gap-4 sm:grid-cols-3">
				{#each [{ label: 'บุคลากรทั้งหมด', count: overview.total, status: 'all', icon: UsersRound, iconClass: 'bg-primary/10 text-primary' }, { label: 'ปฏิบัติงาน', count: overview.active, status: 'active', icon: UserRoundCheck, iconClass: 'bg-chart-2/10 text-chart-2' }, { label: 'สถานะอื่น', count: overview.otherStatuses, status: null, icon: UserRoundMinus, iconClass: 'bg-muted text-muted-foreground' }] as card (card.label)}
					<div class="flex flex-col gap-4 rounded-xl border bg-card p-5">
						<div class="flex flex-1 items-start justify-between gap-4">
							<div class="space-y-2">
								<p class="text-sm font-medium text-muted-foreground">{card.label}</p>
								<p
									class="flex items-baseline gap-2 text-3xl font-semibold tracking-tight tabular-nums"
								>
									{card.count.toLocaleString('th-TH')}
									<span class="text-sm font-normal text-muted-foreground">คน</span>
								</p>
							</div>
							<div
								class={`flex size-11 shrink-0 items-center justify-center rounded-xl ${card.iconClass}`}
								aria-hidden="true"
							>
								<card.icon class="size-5" />
							</div>
						</div>
						<div class="flex min-h-12 items-center border-t pt-4">
							{#if card.status}
								<Button
									variant="secondary"
									size="sm"
									href={`/staff/manage?status=${card.status}`}
									aria-label={`ดูรายชื่อ${card.label}`}
									data-sveltekit-preload-data="tap"
								>
									ดูรายชื่อ
									<ArrowUpRight class="size-4" aria-hidden="true" />
								</Button>
							{:else}
								<p class="text-xs leading-relaxed text-muted-foreground">
									รวมปิดใช้งาน ระงับ ลาออก และเกษียณ
								</p>
							{/if}
						</div>
					</div>
				{/each}
			</div>
			<PersonnelStatusChart buckets={overview.statuses} total={overview.total} />
			<div class="rounded-xl border bg-card p-3 sm:p-4 flex flex-wrap items-center gap-3">
				<p class="text-sm font-medium">สถานะสำหรับกราฟด้านล่าง</p>
				<Select.Root
					type="single"
					value={data.status}
					onValueChange={(value) => {
						void goto(resolve(`/staff/manage/overview?status=${value}`));
					}}
					><Select.Trigger class="w-full sm:w-48" aria-label="สถานะสำหรับกราฟ"
						>{data.status === 'all' ? 'ทุกสถานะ' : staffStatusLabel(data.status)}</Select.Trigger
					><Select.Content
						><Select.Item value="all">ทุกสถานะ</Select.Item
						>{#each STAFF_STATUS_OPTIONS as option (option.value)}<Select.Item value={option.value}
								>{option.label}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
				<p class="text-sm text-muted-foreground">
					{overview.filteredTotal} คน {#if loading}<span role="status">กำลังอัปเดต…</span>{/if}
				</p>
			</div>
			<div class="grid gap-5 lg:grid-cols-2">
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
</PageShell>
