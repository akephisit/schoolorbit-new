<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		getTermActivationWorkspace,
		getTermLifecycleWorkspace,
		getYearLifecycleWorkspace,
		transitionAcademicTerm,
		transitionAcademicYear,
		type TermActivationWorkspace,
		type TermLifecycleWorkspace,
		type YearLifecycleWorkspace,
		type TermTransitionRequest,
		type TermTransitionOutcome,
		type YearTransitionRequest
	} from '#lib/api/academic-lifecycle.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	type Workspace =
		| { kind: 'activate'; data: TermActivationWorkspace }
		| { kind: 'close_term'; data: TermLifecycleWorkspace }
		| { kind: 'close_year'; data: YearLifecycleWorkspace };
	let {
		yearId,
		termId = '',
		kind,
		label,
		disabled = false,
		oncompleted,
		onactivated
	}: {
		yearId: string;
		termId?: string;
		kind: Workspace['kind'];
		label: string;
		disabled?: boolean;
		oncompleted: () => Promise<void>;
		onactivated?: (outcome: TermTransitionOutcome) => void | Promise<void>;
	} = $props();
	let open = $state(false);
	let loading = $state(false);
	let busy = $state(false);
	let error = $state('');
	let stale = $state(false);
	let workspace = $state.raw<Workspace | null>(null);
	let acknowledged = $state<string[]>([]);
	let pending = $state<
		| { kind: 'term'; input: TermTransitionRequest }
		| { kind: 'year'; input: YearTransitionRequest }
		| null
	>(null);
	const latest = new LatestRequest();
	let alive = false;
	const permitted = $derived(
		$can.hasAll(
			PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL,
			kind === 'activate'
				? PERMISSIONS.ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL
				: PERMISSIONS.ACADEMIC_LIFECYCLE_CLOSE_SCHOOL
		)
	);
	const warnings = $derived(
		workspace?.data.findings.filter((finding) => finding.severity === 'warning') ?? []
	);
	const ready = $derived(
		!!workspace &&
			!workspace.data.findings.some((finding) => finding.severity === 'blocking') &&
			(workspace.kind === 'activate'
				? workspace.data.canActivate
				: workspace.kind === 'close_year'
					? workspace.data.canClose
					: workspace.data.availableActions.includes('close'))
	);
	const confirmable = $derived(
		permitted &&
			ready &&
			!busy &&
			!loading &&
			!stale &&
			(kind === 'activate' || warnings.every((finding) => acknowledged.includes(finding.code)))
	);
	async function load() {
		if (!permitted || busy) return;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		loading = true;
		error = '';
		try {
			const result: Workspace =
				kind === 'activate'
					? {
							kind,
							data: await getTermActivationWorkspace(termId, { academicYearId: yearId }, { signal })
						}
					: kind === 'close_term'
						? {
								kind,
								data: await getTermLifecycleWorkspace(
									termId,
									{ academicYearId: yearId },
									{ signal }
								)
							}
						: { kind, data: await getYearLifecycleWorkspace(yearId, { signal }) };
			if (!latest.isCurrent(revision) || identity !== appIdentityKey()) return;
			workspace = result;
			pending = null;
			acknowledged = [];
			stale = false;
		} catch (cause) {
			if (!isAbortError(cause) && latest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	function changeOpen(value: boolean) {
		if (busy) return;
		open = value;
		if (value) {
			workspace = null;
			pending = null;
			void load();
		} else {
			latest.abort();
			loading = false;
		}
	}
	async function confirm() {
		if (!confirmable || !workspace) return;
		if (!pending) {
			const common = {
				requestId: crypto.randomUUID(),
				readinessChecksum: workspace.data.sourceChecksum,
				acknowledgedWarningCodes: kind === 'activate' ? [] : [...acknowledged].sort()
			};
			pending =
				workspace.kind === 'close_year'
					? {
							kind: 'year',
							input: {
								...common,
								action: 'close',
								expectedYearVersion: workspace.data.context.rowVersion
							}
						}
					: {
							kind: 'term',
							input: {
								...common,
								action: workspace.kind === 'activate' ? 'activate' : 'close',
								academicYearId: yearId,
								expectedTermVersion: workspace.data.context.termRowVersion,
								expectedYearVersion: workspace.data.context.yearRowVersion,
								reason: null
							}
						};
		}
		const request = pending;
		const identity = appIdentityKey();
		busy = true;
		error = '';
		try {
			const outcome =
				request.kind === 'year'
					? await transitionAcademicYear(yearId, request.input)
					: await transitionAcademicTerm(termId, request.input);
			if (!alive || identity !== appIdentityKey()) return;
			pending = null;
			open = false;
			if (outcome.action === 'activate') await onactivated?.(outcome);
			await oncompleted();
		} catch (cause) {
			if (!alive || identity !== appIdentityKey()) return;
			error = cause instanceof Error ? cause.message : 'ยังยืนยันผลการบันทึกไม่ได้';
			if (cause instanceof ApiClientError && cause.status === 409) {
				stale = true;
				pending = null;
			}
		} finally {
			busy = false;
		}
	}
	onMount(() => {
		alive = true;
		let identity = appIdentityKey();
		const unsubscribeIdentity = authStore.subscribe(() => {
			const next = appIdentityKey();
			if (identity && identity !== next) {
				latest.abort();
				open = false;
				workspace = null;
				pending = null;
			}
			identity = next;
		});
		const unregister = registerAcademicContextDirtySource(
			`period:${kind}:${termId || yearId}`,
			() => open || busy
		);
		const unsubscribe = can.subscribe((permissions) => {
			if (!permissions.has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL)) {
				open = false;
				workspace = null;
				pending = null;
				latest.abort();
			}
		});
		return () => {
			alive = false;
			latest.abort();
			unregister();
			unsubscribe();
			unsubscribeIdentity();
		};
	});
</script>

{#if permitted}<Button
		variant={kind === 'activate' ? 'default' : 'outline'}
		{disabled}
		onclick={() => changeOpen(true)}>{label}</Button
	>{/if}
<Dialog.Root {open} onOpenChange={changeOpen}
	><Dialog.Content
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
		><Dialog.Header
			><Dialog.Title>{label}</Dialog.Title><Dialog.Description
				>{kind === 'activate'
					? 'เริ่มใช้ภาคเรียนที่เลือก ภายในปีเดียวกันนักเรียนยังอยู่ชั้นและห้องเดิม'
					: 'ปิดรอบการใช้งานและเก็บข้อมูลเดิมไว้ การปิดไม่สั่งลบรายละเอียด และไม่ต้องยืนยันผลการเรียนก่อน'}</Dialog.Description
			></Dialog.Header
		>{#if loading && !workspace}<PageSkeleton variant="cards" rows={2} />{/if}{#if workspace}<p
				class="font-medium"
			>
				{workspace.kind === 'close_year'
					? workspace.data.context.name
					: workspace.data.context.termName}
			</p>
			{#each workspace.data.findings as finding (finding.code)}<div
					class="space-y-2 rounded-lg border p-3"
				>
					<p class={finding.severity === 'blocking' ? 'text-sm text-destructive' : 'text-sm'}>
						{finding.message}
					</p>
					{#if kind !== 'activate' && finding.severity === 'warning'}<div
							class="flex items-center gap-2"
						>
							<Checkbox
								id={`ack-${kind}-${finding.code}`}
								checked={acknowledged.includes(finding.code)}
								disabled={busy || !!pending}
								onCheckedChange={(checked) => {
									acknowledged = checked
										? [...acknowledged, finding.code]
										: acknowledged.filter((code) => code !== finding.code);
								}}
							/><Label for={`ack-${kind}-${finding.code}`}>รับทราบ</Label>
						</div>{/if}
				</div>{/each}{/if}{#if error}<p role="alert" class="text-sm text-destructive">
				{error}
			</p>{/if}{#if stale || (!workspace && !loading)}<LoadingButton
				variant="outline"
				{loading}
				onclick={() => void load()}>ลองตรวจอีกครั้ง</LoadingButton
			>{/if}{#if pending && error}<p class="text-sm text-muted-foreground">
				ลองยืนยันอีกครั้งจะใช้คำขอเดิม เพื่อป้องกันรายการซ้ำ
			</p>{/if}<Dialog.Footer
			><Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ยกเลิก</Button
			><LoadingButton loading={busy} disabled={!confirmable} onclick={() => void confirm()}
				>ยืนยัน{label}</LoadingButton
			></Dialog.Footer
		></Dialog.Content
	></Dialog.Root
>
