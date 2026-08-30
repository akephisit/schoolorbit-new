<script lang="ts">
	import { onMount } from 'svelte';
	import { ApiClientError } from '$lib/api/client';
	import {
		addDatedRosterMembership,
		endDatedRosterMembership,
		listDatedRosterMemberships,
		previewLearningGroupRoster,
		type DatedRosterMembership,
		type LearningGroup,
		type RosterPreview
	} from '$lib/api/learning-delivery';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Label } from '$lib/components/ui/label';
	import { CalendarMinus, CalendarPlus, History, Plus, X } from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	let {
		group,
		canManage,
		onGroupChanged
	}: {
		group: LearningGroup;
		canManage: boolean;
		onGroupChanged: () => void | Promise<void>;
	} = $props();

	let memberships = $state.raw<DatedRosterMembership[]>([]);
	let rosterPreview = $state.raw<RosterPreview | null>(null);
	let loading = $state(true);
	let loadingCandidates = $state(false);
	let saving = $state(false);
	let addFormOpen = $state(false);
	let studentAcademicYearId = $state('');
	let joinedAt = $state('');
	let endingMembershipId = $state('');
	let leftAt = $state('');
	let errorMessage = $state('');
	let candidateError = $state('');

	let activeStudentYearIds = $derived(
		new Set(
			memberships
				.filter((membership) => !membership.leftAt)
				.map((membership) => membership.studentAcademicYearId)
		)
	);
	let candidateOptions = $derived(
		(rosterPreview?.students ?? [])
			.filter((student) => !activeStudentYearIds.has(student.studentAcademicYearId))
			.map((student) => ({
				id: student.studentAcademicYearId,
				label: student.displayName,
				description: [student.studentCode, student.gradeLevelName, student.homeroomName]
					.filter(Boolean)
					.join(' · ')
			}))
	);

	function todayIso(): string {
		const now = new Date();
		return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
	}

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function membershipState(membership: DatedRosterMembership): 'upcoming' | 'active' | 'ended' {
		const today = todayIso();
		if (membership.joinedAt > today) return 'upcoming';
		if (membership.leftAt && membership.leftAt < today) return 'ended';
		return 'active';
	}

	function stateLabel(membership: DatedRosterMembership): string {
		const state = membershipState(membership);
		return state === 'upcoming'
			? 'กำลังจะเริ่ม'
			: state === 'active'
				? 'กำลังเรียน'
				: 'สิ้นสุดแล้ว';
	}

	function stateClass(membership: DatedRosterMembership): string {
		const state = membershipState(membership);
		if (state === 'upcoming') return 'border-blue-500/35 bg-blue-500/10 text-blue-700';
		if (state === 'active') return 'border-emerald-500/35 bg-emerald-500/10 text-emerald-700';
		return 'border-muted-foreground/25 bg-muted text-muted-foreground';
	}

	async function loadHistory() {
		loading = true;
		errorMessage = '';
		try {
			memberships = await listDatedRosterMemberships(group.id);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'โหลดประวัติรายชื่อนักเรียนไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	async function showAddForm() {
		if (!canManage) return;
		addFormOpen = true;
		if (rosterPreview || loadingCandidates) return;
		loadingCandidates = true;
		candidateError = '';
		try {
			rosterPreview = await previewLearningGroupRoster(group.id);
		} catch (error) {
			candidateError =
				error instanceof Error ? error.message : 'โหลดรายชื่อนักเรียนให้เลือกไม่สำเร็จ';
		} finally {
			loadingCandidates = false;
		}
	}

	async function recoverFromConflict() {
		try {
			await onGroupChanged();
			await loadHistory();
			errorMessage =
				'ข้อมูลกลุ่มถูกแก้ไขจากที่อื่น ระบบโหลดข้อมูลล่าสุดแล้ว กรุณาตรวจสอบและลองใหม่';
		} catch (error) {
			errorMessage =
				error instanceof Error
					? `ข้อมูลกลุ่มเปลี่ยน และโหลดข้อมูลล่าสุดไม่สำเร็จ (${error.message})`
					: 'ข้อมูลกลุ่มเปลี่ยน และโหลดข้อมูลล่าสุดไม่สำเร็จ';
		}
	}

	async function addMembership(event: SubmitEvent) {
		event.preventDefault();
		if (!canManage) return;
		if (!studentAcademicYearId || !joinedAt) return;
		saving = true;
		errorMessage = '';
		try {
			const created = await addDatedRosterMembership(group.id, {
				groupRowVersion: group.rowVersion,
				studentAcademicYearId,
				joinedAt
			});
			memberships = [...memberships, created].sort((left, right) =>
				`${left.displayName}:${left.joinedAt}`.localeCompare(
					`${right.displayName}:${right.joinedAt}`,
					'th-TH',
					{ numeric: true }
				)
			);
			studentAcademicYearId = '';
			joinedAt = '';
			addFormOpen = false;
			await onGroupChanged();
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict();
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'เพิ่มนักเรียนเข้ากลุ่มไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	async function endMembership(membership: DatedRosterMembership) {
		if (!canManage) return;
		if (!leftAt) return;
		saving = true;
		errorMessage = '';
		try {
			const updated = await endDatedRosterMembership(group.id, membership.id, {
				groupRowVersion: group.rowVersion,
				membershipRowVersion: membership.rowVersion,
				leftAt
			});
			memberships = memberships.map((item) => (item.id === updated.id ? updated : item));
			endingMembershipId = '';
			leftAt = '';
			await onGroupChanged();
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await recoverFromConflict();
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'กำหนดวันสิ้นสุดไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	onMount(() => {
		void loadHistory();
	});
</script>

<section class="overflow-hidden rounded-2xl border bg-card">
	<header class="flex flex-wrap items-start justify-between gap-4 border-b bg-muted/20 p-4">
		<div class="flex items-start gap-3">
			<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><History class="size-5" /></div>
			<div>
				<h3 class="font-semibold">ประวัติสมาชิกกลุ่มเรียน</h3>
				<p class="mt-1 text-sm text-muted-foreground">
					หลังเผยแพร่แล้ว ให้เพิ่มหรือกำหนดวันสิ้นสุดเป็นรายคน โดยเก็บประวัติเดิมไว้
				</p>
			</div>
		</div>
		{#if canManage}
			<Button variant="outline" onclick={showAddForm}>
				<Plus class="size-4" /> เพิ่มนักเรียนตามวันที่
			</Button>
		{/if}
	</header>

	{#if addFormOpen && canManage}
		<form
			class="grid gap-3 border-b bg-primary/[0.025] p-4 sm:grid-cols-[minmax(0,1fr)_220px_auto] sm:items-end"
			onsubmit={addMembership}
		>
			<div class="space-y-2">
				<Label>นักเรียน</Label>
				{#if loadingCandidates}
					<div class="h-9 animate-pulse rounded-md bg-muted"></div>
				{:else}
					<DeliveryOptionCombobox
						bind:value={studentAcademicYearId}
						options={candidateOptions}
						placeholder="เลือกนักเรียนจากห้องต้นทาง"
						searchPlaceholder="ค้นหารหัสหรือชื่อ..."
					/>
				{/if}
				{#if candidateError}<p role="alert" class="text-xs text-destructive">
						{candidateError}
					</p>{/if}
			</div>
			<div class="space-y-2">
				<Label for="roster-joined-at">วันที่เริ่มเรียน (joinedAt)</Label>
				<DatePicker
					id="roster-joined-at"
					bind:value={joinedAt}
					placeholder="เลือกวันที่เริ่ม"
					required
				/>
			</div>
			<div class="flex gap-2">
				<LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังเพิ่ม"
					disabled={!studentAcademicYearId || !joinedAt}
				>
					<CalendarPlus class="size-4" /> เพิ่ม
				</LoadingButton>
				<Button
					type="button"
					size="icon"
					variant="ghost"
					onclick={() => (addFormOpen = false)}
					aria-label="ปิดแบบฟอร์ม"><X class="size-4" /></Button
				>
			</div>
		</form>
	{/if}

	<div class="p-4">
		<p
			class="mb-3 rounded-lg border border-blue-500/20 bg-blue-500/5 px-3 py-2 text-xs leading-relaxed text-blue-800"
		>
			วันสิ้นสุดเป็นแบบ <strong>นับรวมวันสิ้นสุด</strong> (inclusive): หากระบุ 15 ก.ย. นักเรียนยังอยู่ในกลุ่มและเห็นตารางของวันที่
			15 ก.ย. แต่ไม่รวมวันที่ 16 ก.ย.
		</p>

		{#if loading}
			<div class="space-y-2" aria-label="กำลังโหลดประวัติสมาชิก">
				<div class="h-16 animate-pulse rounded-xl bg-muted"></div>
				<div class="h-16 animate-pulse rounded-xl bg-muted"></div>
			</div>
		{:else if errorMessage && memberships.length === 0}
			<PageState
				variant="error"
				title="โหลดประวัติสมาชิกไม่สำเร็จ"
				description={errorMessage}
				actionLabel="ลองอีกครั้ง"
				onaction={loadHistory}
			/>
		{:else if memberships.length === 0}
			<PageState
				variant="empty"
				title="ยังไม่มีประวัติสมาชิก"
				description="เผยแพร่รายชื่อเริ่มต้น หรือเพิ่มนักเรียนตามวันที่"
			/>
		{:else}
			<div class="divide-y rounded-xl border">
				{#each memberships as membership (membership.id)}
					<div class="p-3 sm:p-4">
						<div class="flex flex-wrap items-start justify-between gap-3">
							<div class="min-w-0">
								<div class="flex flex-wrap items-center gap-2">
									<p class="font-medium">{membership.displayName}</p>
									<Badge variant="outline" class={stateClass(membership)}
										>{stateLabel(membership)}</Badge
									>
								</div>
								<p class="mt-1 text-sm text-muted-foreground">
									{membership.studentCode ?? 'ไม่มีรหัสนักเรียน'} · เริ่ม {formatDate(
										membership.joinedAt
									)}
									{membership.leftAt
										? ` · สิ้นสุด ${formatDate(membership.leftAt)} (รวมวันนี้)`
										: ' · ยังไม่กำหนดวันสิ้นสุด'}
								</p>
							</div>
							{#if canManage && !membership.leftAt}
								<Button
									size="sm"
									variant="outline"
									onclick={() => {
										endingMembershipId = membership.id;
										leftAt = '';
									}}
								>
									<CalendarMinus class="size-4" /> กำหนดวันสิ้นสุด
								</Button>
							{/if}
						</div>
						{#if endingMembershipId === membership.id && canManage}
							<div
								class="mt-3 grid gap-3 rounded-lg border bg-muted/25 p-3 sm:grid-cols-[220px_auto] sm:items-end"
							>
								<div class="space-y-2">
									<Label for={`roster-left-at-${membership.id}`}
										>วันสุดท้ายที่ยังเรียน (leftAt)</Label
									>
									<DatePicker
										id={`roster-left-at-${membership.id}`}
										bind:value={leftAt}
										placeholder="เลือกวันสิ้นสุด"
										required
									/>
								</div>
								<div class="flex gap-2">
									<LoadingButton
										loading={saving}
										loadingLabel="กำลังบันทึก"
										disabled={!leftAt}
										onclick={() => endMembership(membership)}>บันทึกวันสิ้นสุด</LoadingButton
									>
									<Button
										variant="ghost"
										onclick={() => {
											endingMembershipId = '';
											leftAt = '';
										}}>ยกเลิก</Button
									>
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
		{#if errorMessage && memberships.length > 0}<p
				role="alert"
				class="mt-3 text-sm text-destructive"
			>
				{errorMessage}
			</p>{/if}
	</div>
</section>
