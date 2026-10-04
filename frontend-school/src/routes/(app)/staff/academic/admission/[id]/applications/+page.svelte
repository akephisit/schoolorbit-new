<script lang="ts">
	import { untrack } from 'svelte';
	import { afterNavigate, goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		containsProtectedIdentifier,
		protectedIdentifierSearchCandidate
	} from '#lib/admission/protected-identifier-search.js';
	import {
		listApplications,
		searchApplicationsByIdentifier,
		verifyApplication,
		rejectApplication,
		deleteApplication,
		unverifyApplication,
		type ApplicationListItem,
		applicationStatusLabel
	} from '#lib/api/admission.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { toast } from 'svelte-sonner';
	import {
		Search,
		Check,
		X,
		Eye,
		Filter,
		LoaderCircle,
		Trash2,
		RotateCcw,
		RefreshCw
	} from '@lucide/svelte';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';

	let { data }: PageProps = $props();

	let id = $derived(data.roundId);
	let statusFilter = $derived(data.status);
	let dateFilter = $state(page.url.searchParams.get('date') ?? '');
	const canReadAdmission = $derived($can.has(PERMISSIONS.ADMISSION_READ_ALL));
	const canManageAdmission = $derived($can.has(PERMISSIONS.ADMISSION_MANAGE_ALL));
	const canVerifyAdmission = $derived($can.has(PERMISSIONS.ADMISSION_VERIFY_ALL));
	const request = new LatestRequest();
	let renderedKey = '';
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(false);
	let loaded = $state(false);
	let error = $state('');
	let searchDraft = $state('');
	let idSearchActive = $state(false);
	let identifierSearch: string | null = null;
	let viewRevision = 0;
	const currentRouteKey = () => `${id}|${statusFilter}|${data.search}`;
	const locationDataKey = (url: URL) =>
		JSON.stringify([
			url.pathname,
			url.searchParams.get('status') ?? '',
			url.searchParams.get('search') ?? ''
		]);

	const displayedApps = $derived(
		dateFilter ? applications.filter((a) => a.createdAt?.slice(0, 10) === dateFilter) : applications
	);

	let showRejectDialog = $state(false);
	let rejectingApp: ApplicationListItem | null = $state(null);
	let rejectReason = $state('');
	let rejecting = $state(false);

	let showDeleteDialog = $state(false);
	let deletingApp: ApplicationListItem | null = $state(null);
	let deleting = $state(false);

	let showUnverifyDialog = $state(false);
	let unverifyingApp: ApplicationListItem | null = $state(null);
	let unverifying = $state(false);

	const statusVariant: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
		submitted: 'secondary',
		verified: 'default',
		scored: 'default',
		absent: 'outline',
		rejected: 'destructive',
		accepted: 'default',
		enrolled: 'default',
		withdrawn: 'outline'
	};

	async function loadApps() {
		if (!canReadAdmission) {
			loading = false;
			return;
		}
		if (!id) return;
		if (identifierSearch !== null) {
			viewRevision += 1;
			applications = [];
			loaded = false;
		}
		identifierSearch = null;
		idSearchActive = false;
		const sourceKey = currentRouteKey();
		const { revision, signal } = request.begin();
		loading = true;
		error = '';
		try {
			const rows = await listApplications(
				id,
				{
					status: statusFilter || undefined,
					search: data.search || undefined
				},
				{ signal }
			);
			if (!request.isCurrent(revision) || sourceKey !== currentRouteKey()) return;
			applications = rows;
			loaded = true;
			idSearchActive = false;
		} catch (e) {
			if (!isAbortError(e) && request.isCurrent(revision))
				error = e instanceof Error ? e.message : 'โหลดไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function searchIdentifier(identifier: string) {
		if (!canReadAdmission || !id) return;
		const sourceKey = currentRouteKey();
		if (identifierSearch !== identifier) {
			viewRevision += 1;
			applications = [];
			loaded = false;
			idSearchActive = false;
		}
		identifierSearch = identifier;
		const { revision, signal } = request.begin();
		searchDraft = '';
		loading = true;
		error = '';
		try {
			const rows = await searchApplicationsByIdentifier(id, identifier, { signal });
			if (!request.isCurrent(revision) || sourceKey !== currentRouteKey()) return;
			applications = statusFilter ? rows.filter((row) => row.status === statusFilter) : rows;
			loaded = true;
			idSearchActive = true;
		} catch (e) {
			if (!isAbortError(e) && request.isCurrent(revision))
				error = e instanceof Error ? e.message : 'ค้นหาใบสมัครไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	function refreshVisibleApps() {
		if (renderedKey !== currentRouteKey()) {
			void loadApps();
			return;
		}
		if (identifierSearch !== null) void searchIdentifier(identifierSearch);
		else void loadApps();
	}

	function mutationNeedsReconciliation(
		sourceLocationKey: string,
		sourceRoundId: string,
		sourceKey: string,
		sourceViewRevision: number
	): boolean {
		if (sourceLocationKey !== locationDataKey(new URL(page.url.href))) {
			const listPath = resolve(`staff/academic/admission/${sourceRoundId}/applications`);
			if (page.url.pathname === listPath) void invalidate('schoolorbit:admission-applications');
			return true;
		}
		if (sourceKey !== currentRouteKey() || sourceViewRevision !== viewRevision) {
			refreshVisibleApps();
			return true;
		}
		return false;
	}

	function commitSearch() {
		const query = searchDraft.trim();
		const identifier = protectedIdentifierSearchCandidate(query);
		if (identifier) {
			void searchIdentifier(identifier);
			return;
		}
		if (containsProtectedIdentifier(query)) {
			toast.error('กรุณากรอกเลข 13 หลักให้ชัดเจน');
			return;
		}
		if (identifierSearch !== null && query === data.search) {
			void loadApps();
			return;
		}
		setFilter('search', query);
	}

	function setFilter(name: 'status' | 'search' | 'date', value: string) {
		const next = new URL(window.location.href);
		if (value) next.searchParams.set(name, value);
		else next.searchParams.delete(name);
		if (next.href !== window.location.href) {
			if (name === 'date') {
				dateFilter = value;
				goto(resolve(`staff/academic/admission/${id}/applications${next.search}`), {
					shallow: true,
					state: page.state
				});

				return;
			}

			if (name === 'status' && identifierSearch !== null)
				toast.info('เปลี่ยนสถานะแล้ว การค้นหาเลขเดิมถูกยกเลิก');

			void goto(resolve(`staff/academic/admission/${id}/applications${next.search}`), {
				reset: false
			});
		}
	}

	function syncDateFilterFromUrl() {
		dateFilter = new URL(window.location.href).searchParams.get('date') ?? '';
	}

	afterNavigate(({ shallow }) => {
		if (!shallow) syncDateFilterFromUrl();
	});

	function supersedeListRead() {
		request.abort();
		loading = false;
		error = '';
	}

	function patchStatus(appId: string, status: string) {
		supersedeListRead();
		applications = applications
			.map((app) => (app.id === appId ? { ...app, status } : app))
			.filter((app) => !statusFilter || app.status === statusFilter);
	}

	async function handleVerify(app: ApplicationListItem) {
		if (!canVerifyAdmission) {
			toast.error('ไม่มีสิทธิ์ตรวจใบสมัคร');
			return;
		}
		const sourceRoundId = id;
		const sourceLocationKey = locationDataKey(new URL(page.url.href));
		const sourceKey = currentRouteKey();
		const sourceViewRevision = viewRevision;
		try {
			await verifyApplication(app.id);
			if (sourceRoundId !== id) return;
			toast.success(`ยืนยัน ${app.fullName} แล้ว`);
			if (
				!mutationNeedsReconciliation(
					sourceLocationKey,
					sourceRoundId,
					sourceKey,
					sourceViewRevision
				)
			)
				patchStatus(app.id, 'verified');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยืนยันไม่สำเร็จ');
		}
	}

	async function handleRejectConfirm() {
		if (!canVerifyAdmission) {
			toast.error('ไม่มีสิทธิ์ปฏิเสธใบสมัคร');
			return;
		}
		if (!rejectingApp || !rejectReason.trim()) return;
		const target = rejectingApp;
		const reason = rejectReason;
		const sourceRoundId = id;
		const sourceLocationKey = locationDataKey(new URL(page.url.href));
		const sourceKey = currentRouteKey();
		const sourceViewRevision = viewRevision;
		rejecting = true;
		try {
			await rejectApplication(target.id, reason);
			if (sourceRoundId !== id) return;
			toast.success('ปฏิเสธใบสมัครแล้ว');
			if (
				!mutationNeedsReconciliation(
					sourceLocationKey,
					sourceRoundId,
					sourceKey,
					sourceViewRevision
				)
			)
				patchStatus(target.id, 'rejected');
			showRejectDialog = false;
			rejectingApp = null;
			rejectReason = '';
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ปฏิเสธไม่สำเร็จ');
		} finally {
			rejecting = false;
		}
	}

	async function handleDeleteConfirm() {
		if (!canManageAdmission) {
			toast.error('ไม่มีสิทธิ์ลบใบสมัคร');
			return;
		}
		if (!deletingApp) return;
		const target = deletingApp;
		const sourceRoundId = id;
		const sourceLocationKey = locationDataKey(new URL(page.url.href));
		const sourceKey = currentRouteKey();
		const sourceViewRevision = viewRevision;
		deleting = true;
		try {
			await deleteApplication(target.id);
			if (sourceRoundId !== id) return;
			toast.success(`ลบใบสมัครของ ${target.fullName} แล้ว`);
			if (
				!mutationNeedsReconciliation(
					sourceLocationKey,
					sourceRoundId,
					sourceKey,
					sourceViewRevision
				)
			) {
				supersedeListRead();
				applications = applications.filter((a) => a.id !== target.id);
			}
			showDeleteDialog = false;
			deletingApp = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}

	async function handleUnverifyConfirm() {
		if (!canVerifyAdmission) {
			toast.error('ไม่มีสิทธิ์ยกเลิกการอนุมัติใบสมัคร');
			return;
		}
		if (!unverifyingApp) return;
		const target = unverifyingApp;
		const sourceRoundId = id;
		const sourceLocationKey = locationDataKey(new URL(page.url.href));
		const sourceKey = currentRouteKey();
		const sourceViewRevision = viewRevision;
		unverifying = true;
		try {
			await unverifyApplication(target.id);
			if (sourceRoundId !== id) return;
			toast.success(`ยกเลิกการอนุมัติ ${target.fullName} แล้ว`);
			if (
				!mutationNeedsReconciliation(
					sourceLocationKey,
					sourceRoundId,
					sourceKey,
					sourceViewRevision
				)
			)
				patchStatus(target.id, 'submitted');
			showUnverifyDialog = false;
			unverifyingApp = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยกเลิกการอนุมัติไม่สำเร็จ');
		} finally {
			unverifying = false;
		}
	}

	function saveNavContext(apps: ApplicationListItem[]) {
		sessionStorage.setItem(
			'admissionAppNav',
			JSON.stringify({ roundId: id, ids: apps.map((app) => app.id) })
		);
	}

	$effect.pre(() => {
		const routeApplications = data.applications;
		const key = `${data.roundId}|${data.status}|${data.search}`;
		const { revision } = request.begin();
		untrack(() => {
			viewRevision += 1;
			identifierSearch = null;
			idSearchActive = false;
			if (renderedKey !== key) {
				applications = [];
				loaded = false;
				renderedKey = key;
			}
			searchDraft = data.search;
			loading = true;
			error = '';
		});
		void routeApplications.then((result) => {
			if (!request.isCurrent(revision)) return;
			untrack(() => {
				if (result.ok) {
					applications = result.data;
					loaded = true;
				} else error = result.error;
				loading = false;
			});
		});
		return () => request.abort();
	});
</script>

<svelte:window onpopstate={syncDateFilterFromUrl} />

<PageShell
	title="ใบสมัคร"
	description="ค้นหา ตรวจสอบ และจัดการสถานะใบสมัครของรอบนี้"
	backHref="/staff/academic/admission/{id}"
>
	{#snippet actions()}
		<Button
			variant="outline"
			size="icon"
			onclick={refreshVisibleApps}
			disabled={loading || !canReadAdmission}
			aria-label="โหลดใบสมัครใหม่"
		>
			<RefreshCw class="size-4" />
		</Button>
	{/snippet}
	{#if !canReadAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูใบสมัคร"
			description="บัญชีนี้เข้า module รับสมัครได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลใบสมัคร"
		/>
	{:else}
		<!-- Filters -->
		<Card.Root class="gap-0 py-0">
			<Card.Content class="p-3 sm:p-4">
				<div class="flex flex-col flex-wrap gap-3 sm:flex-row">
					<div class="relative flex-1 min-w-0 sm:min-w-48">
						<Search
							class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground"
						/>
						<Input
							bind:value={searchDraft}
							placeholder="ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร..."
							class="pl-9"
							onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && commitSearch()}
						/>
					</div>
					<div class="w-full sm:w-44">
						<Select.Root
							type="single"
							value={statusFilter}
							onValueChange={(value) => setFilter('status', value ?? '')}
						>
							<Select.Trigger>
								{statusFilter ? applicationStatusLabel[statusFilter] : 'สถานะทั้งหมด'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">สถานะทั้งหมด</Select.Item>
								<Select.Item value="submitted">รอตรวจสอบ</Select.Item>
								<Select.Item value="verified">ผ่านตรวจสอบ</Select.Item>
								<Select.Item value="scored">กรอกคะแนนแล้ว</Select.Item>
								<Select.Item value="absent">ขาดสอบ</Select.Item>
								<Select.Item value="rejected">ไม่ผ่าน</Select.Item>
								<Select.Item value="accepted">ได้รับคัดเลือก</Select.Item>
								<Select.Item value="enrolled">มอบตัวแล้ว</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
					<div class="flex items-center gap-1.5 flex-1 sm:flex-none sm:w-48">
						<DatePicker
							value={dateFilter}
							onValueChange={(value) => setFilter('date', value ?? '')}
							placeholder="กรองตามวันที่"
							class="w-full"
						/>
						{#if dateFilter}
							<Button
								variant="ghost"
								size="icon"
								class="h-9 w-9 shrink-0"
								onclick={() => setFilter('date', '')}
								title="ล้างวันที่"
							>
								<X class="w-3.5 h-3.5" />
							</Button>
						{/if}
					</div>
					<Button onclick={commitSearch} variant="outline" class="w-full gap-1.5 sm:w-auto">
						<Filter class="w-4 h-4" /> ค้นหา
					</Button>
				</div>
			</Card.Content>
		</Card.Root>
		{#if idSearchActive}
			<p role="status" class="text-sm text-muted-foreground">
				ผลค้นหาเลขบัตรหรือเลขที่ใบสมัคร (ไม่บันทึกเลขที่ค้นหาใน URL;
				เปลี่ยนสถานะแล้วจะออกจากผลค้นหานี้)
				<Button variant="outline" size="sm" onclick={loadApps}>กลับรายการทั้งหมด</Button>
			</p>
		{/if}

		<!-- Table -->
		{#if loading && !loaded}
			<PageSkeleton variant="table" rows={6} columns={5} />
		{:else if error && !loaded}
			<PageState
				variant="error"
				title="โหลดใบสมัครไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={refreshVisibleApps}
			/>
		{:else}
			<div aria-busy={loading}>
				{#if loading}<RegionUpdatingState class="static" label="กำลังอัปเดตใบสมัคร..." />{/if}
				{#if error}<p role="alert" class="text-sm text-destructive">
						{error}
						<Button variant="outline" size="sm" onclick={refreshVisibleApps}>ลองใหม่</Button>
					</p>{/if}
				{#if applications.length === 0}
					<PageState title="ไม่พบใบสมัคร" description="ยังไม่มีใบสมัครที่ตรงกับเงื่อนไขการค้นหา" />
				{:else}
					<Card.Root>
						<div class="overflow-x-auto">
							<Table.Root>
								<Table.Header>
									<Table.Row>
										<Table.Head class="w-24">เลขที่</Table.Head>
										<Table.Head>ชื่อ-สกุล</Table.Head>
										<Table.Head>สาย</Table.Head>
										<Table.Head>สถานะ</Table.Head>
										<Table.Head class="text-right">จัดการ</Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each displayedApps as app (app.id)}
										<Table.Row>
											<Table.Cell class="font-mono text-xs"
												>{app.applicationNumber ?? '-'}</Table.Cell
											>
											<Table.Cell>
												<p class="font-medium">{app.fullName}</p>
												<p class="text-xs text-muted-foreground">{app.phone ?? ''}</p>
											</Table.Cell>
											<Table.Cell class="text-sm">{app.trackName ?? '-'}</Table.Cell>
											<Table.Cell>
												<Badge variant={statusVariant[app.status] ?? 'outline'}>
													{applicationStatusLabel[app.status] ?? app.status}
												</Badge>
											</Table.Cell>
											<Table.Cell class="text-right">
												<div class="flex justify-end gap-1">
													<Button
														href="/staff/academic/admission/{id}/applications/{app.id}"
														data-sveltekit-preload-data="tap"
														variant="ghost"
														size="icon"
														class="h-8 w-8"
														onclick={() => saveNavContext(displayedApps)}
													>
														<Eye class="w-3.5 h-3.5" />
													</Button>
													{#if canVerifyAdmission && app.status === 'submitted'}
														<Button
															variant="ghost"
															size="icon"
															class="h-8 w-8 text-green-600 hover:text-green-700"
															onclick={() => handleVerify(app)}
															title="อนุมัติ"
														>
															<Check class="w-3.5 h-3.5" />
														</Button>
														<Button
															variant="ghost"
															size="icon"
															class="h-8 w-8 text-destructive hover:text-destructive"
															onclick={() => {
																rejectingApp = app;
																showRejectDialog = true;
															}}
															title="ไม่อนุมัติ"
														>
															<X class="w-3.5 h-3.5" />
														</Button>
													{/if}
													{#if canVerifyAdmission && app.status === 'verified'}
														<Button
															variant="ghost"
															size="icon"
															class="h-8 w-8 text-muted-foreground hover:text-destructive"
															onclick={() => {
																unverifyingApp = app;
																showUnverifyDialog = true;
															}}
															title="ยกเลิกการอนุมัติ"
														>
															<RotateCcw class="w-3.5 h-3.5" />
														</Button>
													{/if}
													{#if canManageAdmission}
														<Button
															variant="ghost"
															size="icon"
															class="h-8 w-8 text-muted-foreground hover:text-destructive"
															onclick={() => {
																deletingApp = app;
																showDeleteDialog = true;
															}}
															title="ลบใบสมัคร"
														>
															<Trash2 class="w-3.5 h-3.5" />
														</Button>
													{/if}
												</div>
											</Table.Cell>
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</div>

						<div class="px-4 py-3 border-t border-border">
							<p class="text-xs text-muted-foreground">
								แสดง {displayedApps.length} จาก {applications.length} รายการ
							</p>
						</div>
					</Card.Root>
				{/if}
			</div>
		{/if}
	{/if}
</PageShell>

<!-- Reject Dialog -->
{#if canVerifyAdmission}
	<Dialog.Root bind:open={showRejectDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ปฏิเสธใบสมัคร</Dialog.Title>
				<Dialog.Description>
					ปฏิเสธใบสมัครของ <strong>{rejectingApp?.fullName}</strong>
				</Dialog.Description>
			</Dialog.Header>
			<div class="space-y-2 py-2">
				<Label for="reject-reason">เหตุผล <span class="text-destructive">*</span></Label>
				<Textarea
					id="reject-reason"
					bind:value={rejectReason}
					placeholder="ระบุเหตุผลที่ปฏิเสธ..."
					rows={3}
				/>
			</div>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showRejectDialog = false)}>ยกเลิก</Button>
				<Button
					variant="destructive"
					onclick={handleRejectConfirm}
					disabled={rejecting || !rejectReason.trim()}
				>
					{#if rejecting}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
					{rejecting ? 'กำลังดำเนินการ...' : 'ปฏิเสธ'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}

<!-- Unverify Dialog -->
{#if canVerifyAdmission}
	<Dialog.Root bind:open={showUnverifyDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยกเลิกการอนุมัติ</Dialog.Title>
				<Dialog.Description>
					ยืนยันการยกเลิกการอนุมัติใบสมัครของ <strong>{unverifyingApp?.fullName}</strong>?
					ใบสมัครจะกลับสู่สถานะ "รอตรวจสอบ"
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showUnverifyDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleUnverifyConfirm} disabled={unverifying}>
					{#if unverifying}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
					{unverifying ? 'กำลังดำเนินการ...' : 'ยืนยัน'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}

<!-- Delete Dialog -->
{#if canManageAdmission}
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ลบใบสมัคร</Dialog.Title>
				<Dialog.Description>
					ลบใบสมัครของ <strong>{deletingApp?.fullName}</strong> ออกจากระบบ การดำเนินการนี้ไม่สามารถยกเลิกได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleDeleteConfirm} disabled={deleting}>
					{#if deleting}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
					{deleting ? 'กำลังลบ...' : 'ยืนยันการลบ'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}
