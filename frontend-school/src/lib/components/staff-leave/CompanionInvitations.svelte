<script lang="ts">
	import { onDestroy } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { invalidate } from '$app/navigation';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		confirmLeaveCompanion,
		listLeaveCompanionInvitations,
		type LeaveCompanionInvitation
	} from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import { leaveKindLabels, leavePeriodCaption } from '#lib/staff-leave/presentation.js';
	import { companionMaxOffset, companionPageSize } from '#lib/staff-leave/filters.js';
	import { leaveCompanionPermissions } from '#lib/staff-leave/route-data.js';
	import { can } from '#lib/stores/permissions.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	let {
		source,
		offset
	}: {
		source: Promise<RouteLoadResult<LeaveCompanionInvitation[]>>;
		offset: number;
	} = $props();
	const region = new LeaveRegion<LeaveCompanionInvitation[]>();
	const intents = new SvelteMap<string, LeaveMutationIntent>();
	const pending = new SvelteSet<string>();
	const confirmedRevisions = new SvelteMap<string, number>();
	const allowed = $derived($can.hasAll(...leaveCompanionPermissions));
	let feedback = $state('');
	let errors = $state<Record<string, string>>({});
	let disposed = false;
	$effect.pre(() => region.consume(source));
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});

	function retry() {
		if (!allowed) return;
		region.retry((signal) => listLeaveCompanionInvitations({ signal, query: { offset } }));
	}

	function isConfirmed(invitation: LeaveCompanionInvitation) {
		return (
			invitation.confirmed || confirmedRevisions.get(invitation.requestId) === invitation.revision
		);
	}

	async function confirm(invitation: LeaveCompanionInvitation) {
		const id = invitation.requestId;
		if (!allowed || isConfirmed(invitation) || pending.has(id)) return;
		pending.add(id);
		delete errors[id];
		const intent = intents.get(id) ?? new LeaveMutationIntent();
		intents.set(id, intent);
		const basis = { expectedRevision: invitation.revision };
		try {
			await confirmLeaveCompanion(id, { requestId: intent.for(basis), ...basis });
			if (disposed) return;
			intents.delete(id);
			confirmedRevisions.set(id, invitation.revision);
			feedback = `ยืนยันร่วมเดินทางกับ ${invitation.ownerName} แล้ว`;
			await invalidate('school:leave-companions');
		} catch (cause) {
			if (disposed) return;
			if (cause instanceof ApiClientError && cause.status === 409) retry();
			errors[id] = cause instanceof Error ? cause.message : 'ยืนยันร่วมเดินทางไม่สำเร็จ';
		} finally {
			if (!disposed) pending.delete(id);
		}
	}
</script>

<PageShell title="คำเชิญร่วมเดินทาง" backHref="/staff/leave" contentClass="space-y-4">
	{#if !allowed}
		<PageState title="ไม่มีสิทธิ์เปิดคำเชิญร่วมเดินทาง" />
	{:else}
		{#if feedback}<p
				role="status"
				class="rounded-md border border-success/25 bg-success/10 px-3 py-2 text-sm text-success"
			>
				{feedback}
			</p>{/if}
		<section aria-label="คำเชิญที่ได้รับ" aria-busy={region.loading} class="space-y-4">
			{#if region.error}
				<PageState
					variant="error"
					title="โหลดคำเชิญไม่สำเร็จ"
					description={region.error}
					actionLabel="ลองอีกครั้ง"
					onaction={retry}
				/>
			{/if}
			{#if region.loading && !region.value}
				<PageSkeleton variant="cards" />
			{:else if region.value?.length}
				<ul class="divide-y rounded-lg border bg-card">
					{#each region.value as invitation (invitation.requestId)}
						<li class="space-y-3 p-4">
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0 space-y-1">
									<h2 class="font-medium">{leaveKindLabels[invitation.kind]}</h2>
									<p class="break-words text-sm">ผู้ขอ {invitation.ownerName}</p>
									<p class="text-sm text-muted-foreground">{leavePeriodCaption(invitation)}</p>
									{#if invitation.destination}<p class="break-words text-sm">
											สถานที่ {invitation.destination}
										</p>{/if}
								</div>
								{#if isConfirmed(invitation)}
									<Badge variant="outline" class="border-success/25 bg-success/10 text-success"
										>ยืนยันแล้ว</Badge
									>
								{:else}
									<LoadingButton
										loading={pending.has(invitation.requestId)}
										onclick={() => confirm(invitation)}>ยืนยันร่วมเดินทาง</LoadingButton
									>
								{/if}
							</div>
							{#if errors[invitation.requestId]}
								<p role="alert" class="text-sm text-destructive">{errors[invitation.requestId]}</p>
							{/if}
						</li>
					{/each}
				</ul>
			{:else if !region.error}
				<PageState title="ยังไม่มีคำเชิญร่วมเดินทาง" />
			{/if}
			{#if offset > 0 || region.value?.length === companionPageSize}
				<nav aria-label="หน้าคำเชิญ" class="flex justify-end gap-2">
					<Button
						variant="outline"
						disabled={!offset}
						href={offset
							? `/staff/leave/companions?offset=${Math.max(0, offset - companionPageSize)}`
							: undefined}>ก่อนหน้า</Button
					>
					<Button
						variant="outline"
						disabled={offset >= companionMaxOffset || region.value?.length !== companionPageSize}
						href={offset < companionMaxOffset && region.value?.length === companionPageSize
							? `/staff/leave/companions?offset=${offset + companionPageSize}`
							: undefined}>ถัดไป</Button
					>
				</nav>
			{/if}
		</section>
	{/if}
</PageShell>
