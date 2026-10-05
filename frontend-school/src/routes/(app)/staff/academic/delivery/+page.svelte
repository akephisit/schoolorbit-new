<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { untrack, onMount, tick } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { registerDeliveryDraftReconcile } from '#lib/academic/delivery-draft-reconcile.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { authStore } from '#lib/stores/auth.js';
	import {
		connectTimetableSocket,
		disconnectTimetableSocket,
		refreshTrigger
	} from '#lib/stores/timetable-socket.js';
	import {
		selectAcademicTermChangeSetSummary,
		summarizeAcademicTermChangeSet,
		type LearningDeliveryRefreshScope
	} from '#lib/academic/learning-delivery-page.js';
	import {
		buildSynchronizedActivityPreparationTarget,
		activityDraftCandidates,
		type SynchronizedActivityPreparationTarget
	} from '#lib/academic/synchronized-activity-delivery.js';
	import {
		getAcademicTermChangeSet,
		getHomeroomDeliveryWorkspace,
		getDeliveryVersion,
		listAcademicTermChangeSets,
		listDeliveryVersions,
		type DeliveryVersionSummary,
		type AcademicTermChangeSet,
		type AcademicTermChangeSetSummary,
		type HomeroomDeliveryWorkspace as HomeroomWorkspace,
		type DeliveryVersion,
		type LearningOfferingOverviewItem
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '#lib/components/academic-workflow/index.js';
	import AcademicChangeSetDialog from '#lib/components/learning-delivery/AcademicChangeSetDialog.svelte';
	import AcademicChangeSetPanel from '#lib/components/learning-delivery/AcademicChangeSetPanel.svelte';
	import HomeroomDeliveryWorkspace from '#lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte';
	import OfferingCreateDialog from '#lib/components/learning-delivery/OfferingCreateDialog.svelte';
	import DeliveryVersionOverviewTable from '#lib/components/learning-delivery/DeliveryVersionOverviewTable.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { RefreshCw } from '@lucide/svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	let { data }: { data: PageData } = $props();
	const academicYearId = $derived(data.context?.academicYearId ?? null);
	const academicTermId = $derived(data.context?.academicTermId ?? null);
	const homeroomRequest = new LatestRequest();
	const changeSetSummaryRequest = new LatestRequest();
	const changeSetRequest = new LatestRequest();
	const overviewRequest = new LatestRequest();
	const deliveryVersionsRequest = new LatestRequest();
	let deliveryVersions = $state<DeliveryVersionSummary[]>([]);
	let deliveryVersionsLoading = $state(false);
	let deliveryVersionsError = $state('');
	const selectedDelivery = $derived(
		deliveryVersions.find((version) => version.id === workspace?.deliveryVersionId) ?? null
	);
	async function refreshDeliveryVersions(): Promise<void> {
		if (!academicTermId) return;
		const { revision, signal } = deliveryVersionsRequest.begin();
		deliveryVersionsLoading = true;
		deliveryVersionsError = '';
		try {
			const loaded = await listDeliveryVersions(academicTermId, { signal });
			if (deliveryVersionsRequest.isCurrent(revision)) deliveryVersions = loaded;
		} catch (error) {
			if (!isAbortError(error) && deliveryVersionsRequest.isCurrent(revision))
				deliveryVersionsError = error instanceof Error ? error.message : 'โหลดรุ่นเปิดสอนไม่สำเร็จ';
		} finally {
			if (deliveryVersionsRequest.isCurrent(revision)) deliveryVersionsLoading = false;
		}
	}
	function deliveryVersionLabel(version: DeliveryVersionSummary): string {
		return `${version.status === 'draft' ? 'แบบร่าง' : version.status === 'published' ? 'เผยแพร่แล้ว' : 'ยกเลิก'} · เริ่มใช้ ${version.effectiveFrom ? formatDate(version.effectiveFrom) : 'เลือกวันตอนเผยแพร่'} · ${version.offeringCount} รายการ`;
	}
	function selectDeliveryVersion(id: string): void {
		const url = new URL(page.url.href);
		url.searchParams.set('deliveryVersionId', id);
		url.searchParams.delete('changeSetId');
		void goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`));
	}
	$effect.pre(() => {
		const promise = data.deliveryVersions;
		const { revision } = deliveryVersionsRequest.begin();
		untrack(() => {
			deliveryVersionsLoading = Boolean(promise);
			deliveryVersionsError = '';
			deliveryVersions = [];
		});
		if (promise)
			void promise.then((result) => {
				if (!deliveryVersionsRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) deliveryVersions = result.data;
					else deliveryVersionsError = result.error;
					deliveryVersionsLoading = false;
				});
			});
		return () => {
			if (deliveryVersionsRequest.isCurrent(revision)) deliveryVersionsRequest.abort();
		};
	});
	let workspace = $state.raw<HomeroomWorkspace | null>(null);
	let overview = $state.raw<DeliveryVersion | null>(null);
	let revisionOfferings = $state.raw<DeliveryVersion['snapshot']['offerings']>([]);
	let changeSets = $state.raw<AcademicTermChangeSetSummary[]>([]);
	let activeChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
	let selectedChangeSetId = $state('');
	let homeroomLoading = $state(false);
	let changeSetSummaryLoading = $state(false);
	let changeSetSummaryResolved = $state(false);
	let changeSetLoading = $state(false);
	let overviewLoading = $state(false);
	let homeroomError = $state('');
	let changeSetSummaryError = $state('');
	let changeSetError = $state('');
	let errorMessage = $state('');
	let viewMode = $state<'homerooms' | 'offerings'>('homerooms');
	let offeringDialog = $state<{
		openCurriculumPreparation: (
			target: SynchronizedActivityPreparationTarget,
			deliveryVersionId?: string | null
		) => Promise<void>;
	}>();
	let deliveryRevisionDialog = $state<{ openDialog: () => void }>();
	let pendingDeliveryAction = $state.raw<{ kind: 'activate'; catalogVersionId: string } | null>(
		null
	);
	let activationBusy = $state(false);
	let activationError = $state('');
	let draftChoices = $state<DeliveryVersionSummary[]>([]);
	let chooseDraftOpen = $state(false);
	let sourceNotice = $state('');
	let activationContext = '';
	$effect(() => {
		const context = `${academicYearId}:${academicTermId}`;
		untrack(() => {
			if (context !== activationContext) {
				activationContext = context;
				pendingDeliveryAction = null;
				activationError = '';
				chooseDraftOpen = false;
			}
		});
	});
	let initialKind = $derived<'all' | 'activity'>(
		page.url.searchParams.get('kind') === 'activity' ? 'activity' : 'all'
	);
	let canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	let items = $derived(overview?.snapshot.offerings ?? []);

	const missingTermPrerequisite: AcademicPrerequisite = {
		key: 'academic-term',
		status: 'missing',
		title: 'เลือกปีการศึกษาและภาคเรียนก่อน',
		description: 'มุมมองรายห้อง รายการเปิดสอน กลุ่ม ครู และตาราง แยกกันในแต่ละภาคเรียน',
		actionLabel: 'ไปตั้งค่าปีและภาคเรียน',
		href: '/staff/academic/core'
	};
	const noOfferingPrerequisite: AcademicPrerequisite = {
		key: 'learning-offerings',
		status: 'warning',
		title: 'ภาคเรียนนี้ยังไม่มีรายการเปิดสอน',
		description: 'นำรายวิชาและกิจกรรมจากหลักสูตรมาใช้ หรือเพิ่มรายการเฉพาะภาคเรียนนี้ได้',
		actionLabel: 'ตรวจหลักสูตรและแผนการเรียน',
		href: '/staff/academic/curricula'
	};

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function updateSelectedChangeSetUrl(id: string) {
		const url = new URL(page.url.href);

		if (id) {
			url.searchParams.set('changeSetId', id);
			const selected = changeSets.find((item) => item.id === id);
			if (selected) url.searchParams.set('deliveryVersionId', selected.targetDeliveryVersionId);
		} else url.searchParams.delete('changeSetId');

		goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`), {
			replace: true,
			state: page.state
		});
	}

	function applyChangeSetSummaries(loaded: AcademicTermChangeSetSummary[]) {
		changeSets = loaded;
		const selected = selectAcademicTermChangeSetSummary(
			loaded,
			page.url.searchParams.get('changeSetId')?.trim() || selectedChangeSetId
		);
		selectedChangeSetId = selected?.id ?? '';
		if (!selected) activeChangeSet = null;
		return selected;
	}

	async function loadHomerooms() {
		if (!academicYearId || !academicTermId) return;
		const { revision, signal } = homeroomRequest.begin();
		homeroomLoading = true;
		homeroomError = '';
		try {
			const result = await getHomeroomDeliveryWorkspace(academicYearId, academicTermId, {
				deliveryVersionId: page.url.searchParams.get('deliveryVersionId') ?? undefined,
				signal
			});
			if (homeroomRequest.isCurrent(revision)) workspace = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (homeroomRequest.isCurrent(revision)) {
				homeroomError = error instanceof Error ? error.message : 'โหลดภาพรวมรายห้องไม่สำเร็จ';
			}
		} finally {
			if (homeroomRequest.isCurrent(revision)) homeroomLoading = false;
		}
	}

	async function loadSelectedChangeSet(id: string, updateUrl = true) {
		if (!id || !academicTermId) {
			activeChangeSet = null;
			changeSetLoading = false;
			return;
		}
		const { revision, signal } = changeSetRequest.begin();
		selectedChangeSetId = id;
		activeChangeSet = null;
		changeSetLoading = true;
		changeSetError = '';
		if (updateUrl) updateSelectedChangeSetUrl(id);
		try {
			const result = await getAcademicTermChangeSet(id, { signal });
			if (result.academicTermId !== academicTermId) {
				throw new Error('ชุดการเปลี่ยนแปลงไม่อยู่ในภาคเรียนที่เลือก');
			}
			if (changeSetRequest.isCurrent(revision)) activeChangeSet = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (changeSetRequest.isCurrent(revision)) {
				changeSetError =
					error instanceof Error ? error.message : 'โหลดรายละเอียดชุดการเปลี่ยนแปลงไม่สำเร็จ';
			}
		} finally {
			if (changeSetRequest.isCurrent(revision)) changeSetLoading = false;
		}
	}

	async function loadChangeSetSummaries() {
		if (!academicTermId) return;
		const { revision, signal } = changeSetSummaryRequest.begin();
		changeSetSummaryLoading = true;
		changeSetSummaryResolved = false;
		changeSetSummaryError = '';
		try {
			const result = await listAcademicTermChangeSets(academicTermId, { signal });
			if (!changeSetSummaryRequest.isCurrent(revision)) return;
			const selected = applyChangeSetSummaries(result);
			changeSetSummaryResolved = true;
			if (selected) await loadSelectedChangeSet(selected.id, false);
		} catch (error) {
			if (isAbortError(error)) return;
			if (changeSetSummaryRequest.isCurrent(revision)) {
				changeSetSummaryError =
					error instanceof Error ? error.message : 'โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ';
			}
		} finally {
			if (changeSetSummaryRequest.isCurrent(revision)) changeSetSummaryLoading = false;
		}
	}

	async function handleDraftDeleted(sourceVersionId: string | null) {
		activeChangeSet = null;
		selectedChangeSetId = '';
		const url = new URL(page.url.href);
		url.searchParams.delete('changeSetId');
		if (sourceVersionId) url.searchParams.set('deliveryVersionId', sourceVersionId);
		else url.searchParams.delete('deliveryVersionId');
		await goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`), {
			replace: true
		});
		await Promise.all([refreshDeliveryVersions(), loadChangeSetSummaries(), loadHomerooms()]);
	}

	async function loadOverview(termId: string) {
		const { revision, signal } = overviewRequest.begin();
		overviewLoading = true;
		try {
			const versionId = workspace?.deliveryVersionId;
			if (!versionId) return;
			const result = await getDeliveryVersion(versionId, { signal });
			if (result.academicTermId !== termId) throw new Error('รุ่นเปิดสอนไม่อยู่ในภาคเรียนที่เลือก');
			const base = result.sourceVersionId
				? await getDeliveryVersion(result.sourceVersionId, { signal })
				: null;
			if (overviewRequest.isCurrent(revision)) {
				overview = result;
				revisionOfferings = [
					...result.snapshot.offerings,
					...(base?.snapshot.offerings.filter(
						(item) => !result.snapshot.offerings.some((current) => current.id === item.id)
					) ?? [])
				];
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (overviewRequest.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดมุมมองรายวิชาไม่สำเร็จ';
		} finally {
			if (overviewRequest.isCurrent(revision)) overviewLoading = false;
		}
	}

	async function ensureOverview() {
		if (!academicTermId || overview || overviewLoading) return;
		await loadOverview(academicTermId);
	}

	async function refreshDeliveryRegions(refreshOverview = viewMode === 'offerings') {
		const requests: Promise<void>[] = [loadHomerooms()];
		if (refreshOverview && academicTermId) requests.push(loadOverview(academicTermId));
		await Promise.all(requests);
	}

	function changeViewMode(value: string) {
		viewMode = value === 'offerings' ? 'offerings' : 'homerooms';
		if (viewMode === 'offerings' && academicTermId && !overview && !overviewLoading)
			void loadOverview(academicTermId);
	}

	function addCreated(_item: LearningOfferingOverviewItem) {
		void refreshDeliveryVersions();
		void refreshDeliveryRegions();
	}

	async function continueActivityInDraft(versionId: string) {
		const pending = pendingDeliveryAction;
		const context = activationContext;
		if (!pending || !canManage || !academicYearId || !academicTermId) return;
		activationBusy = true;
		activationError = '';
		chooseDraftOpen = false;
		await tick();
		try {
			let selected = workspace;
			if (selected?.deliveryVersionId !== versionId) {
				const url = new URL(page.url.href);
				url.searchParams.set('deliveryVersionId', versionId);
				url.searchParams.delete('changeSetId');
				await goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`));
				await tick();
				const result = await data.homerooms;
				if (!result?.ok) throw new Error(result?.error ?? 'โหลดแบบร่างไม่สำเร็จ');
				selected = result.data;
			}
			if (context !== activationContext || !canManage || pending !== pendingDeliveryAction) return;
			if (
				!selected ||
				selected.deliveryVersionStatus !== 'draft' ||
				selected.deliveryVersionId !== versionId
			)
				throw new Error('แบบร่างนี้ไม่พร้อมแก้ไข กรุณาโหลดล่าสุดแล้วลองอีกครั้ง');
			const existing = selected.homerooms
				.flatMap((room) => room.items)
				.find(
					(item) =>
						item.catalogVersionId === pending.catalogVersionId &&
						item.resourceKind === 'activity' &&
						item.offeringId
				);
			if (existing?.offeringId) {
				await goto(
					resolve(`staff/academic/delivery/${existing.offeringId}?deliveryVersionId=${versionId}`)
				);
			} else {
				const target = buildSynchronizedActivityPreparationTarget(
					selected,
					pending.catalogVersionId
				);
				if (!target || !offeringDialog)
					throw new Error('ไม่พบกิจกรรมในหลักสูตรของร่างนี้ กรุณาตรวจรายการอีกครั้ง');
				await offeringDialog.openCurriculumPreparation(target, versionId);
			}
			pendingDeliveryAction = null;
			chooseDraftOpen = false;
		} catch (error) {
			if (context === activationContext)
				activationError = error instanceof Error ? error.message : 'เปิดกิจกรรมในร่างไม่สำเร็จ';
		} finally {
			activationBusy = false;
		}
	}

	async function prepareSynchronizedActivity(catalogVersionId: string) {
		if (activationBusy || !workspace || !canManage || !academicTermId) return;
		pendingDeliveryAction = { kind: 'activate', catalogVersionId };
		activationError = '';
		if (workspace.deliveryVersionStatus === 'draft' && workspace.deliveryVersionId) {
			await continueActivityInDraft(workspace.deliveryVersionId);
			return;
		}
		const context = activationContext;
		activationBusy = true;
		try {
			const versions = await listDeliveryVersions(academicTermId);
			if (context !== activationContext || !canManage) return;
			const { source, drafts } = activityDraftCandidates(versions);
			if (!source) throw new Error('ไม่พบรุ่นเปิดสอนที่เผยแพร่แล้ว กรุณาสร้างรุ่นเปิดสอนก่อน');
			deliveryVersions = versions;
			sourceNotice =
				workspace.deliveryVersionId !== source.id
					? 'กำลังดูรุ่นย้อนหลัง ร่างสำหรับเพิ่มกิจกรรมจะใช้ข้อมูลจากรุ่นเผยแพร่ล่าสุด'
					: 'เพิ่มกิจกรรมในร่างจากรุ่นเผยแพร่ล่าสุด รุ่นที่เผยแพร่แล้วจะคงข้อมูลเดิม';
			draftChoices = drafts;
			if (drafts.length === 1) {
				if (workspace.deliveryVersionId !== source.id) toast.info(sourceNotice);
				await continueActivityInDraft(drafts[0].id);
			} else if (drafts.length > 1) chooseDraftOpen = true;
			else if (deliveryRevisionDialog) deliveryRevisionDialog.openDialog();
			else throw new Error('หน้าต่างสร้างร่างยังไม่พร้อม กรุณาลองอีกครั้ง');
		} catch (error) {
			if (context === activationContext)
				activationError = error instanceof Error ? error.message : 'ค้นหาแบบร่างไม่สำเร็จ';
		} finally {
			activationBusy = false;
		}
	}

	async function handleDeliveryRevisionCreated(created: AcademicTermChangeSet) {
		if (pendingDeliveryAction) await continueActivityInDraft(created.targetDeliveryVersionId);
		else addChangeSet(created);
	}

	function addChangeSet(created: AcademicTermChangeSet) {
		const summary = summarizeAcademicTermChangeSet(created);
		changeSets = [summary, ...changeSets.filter((changeSet) => changeSet.id !== created.id)];
		selectedChangeSetId = created.id;
		activeChangeSet = created;
		void refreshDeliveryVersions();
		updateSelectedChangeSetUrl(created.id);
	}

	async function updateChangeSet(
		updated: AcademicTermChangeSet,
		refreshScope: LearningDeliveryRefreshScope = 'local'
	) {
		selectedChangeSetId = updated.id;
		activeChangeSet = updated;
		const summary = summarizeAcademicTermChangeSet(updated);

		changeSets = changeSets
			.map((changeSet) => (changeSet.id === updated.id ? summary : changeSet))
			.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));

		if (updated.items.length > 0 && academicTermId && (overview || viewMode === 'offerings')) {
			await loadOverview(academicTermId);
		}
		await refreshDeliveryVersions();
		if (refreshScope === 'homerooms') await loadHomerooms();
	}

	$effect.pre(() => {
		const routeResult = data.homerooms;
		const { revision } = homeroomRequest.begin();
		untrack(() => {
			workspace = null;
			homeroomLoading = Boolean(routeResult);
			homeroomError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!homeroomRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) workspace = result.data;
					else homeroomError = result.error;
					homeroomLoading = false;
				});
			});
		}
		return () => {
			if (homeroomRequest.isCurrent(revision)) homeroomRequest.abort();
		};
	});

	let reconcilingDraft = false;
	async function reconcileOpenDraft() {
		const selected = activeChangeSet;
		if (!selected || selected.status === 'published' || reconcilingDraft || document.hidden) return;
		reconcilingDraft = true;
		try {
			const current = await getAcademicTermChangeSet(selected.id);
			if (activeChangeSet?.id === selected.id && current.rowVersion !== selected.rowVersion) {
				await updateChangeSet(current);
				toast.info('ร่างเปิดสอนเปลี่ยนแล้ว โหลดข้อมูลล่าสุดให้แล้ว');
			}
		} catch (error) {
			if (activeChangeSet?.id !== selected.id) return;
			if (error instanceof ApiClientError && error.status === 404) {
				toast.info('ร่างเปิดสอนนี้ถูกลบแล้ว กลับไปดูรุ่นต้นทาง');
				await handleDraftDeleted(selected.baseDeliveryVersionId ?? null);
			} else {
				changeSetError = error instanceof Error ? error.message : 'โหลดร่างล่าสุดไม่สำเร็จ';
			}
		} finally {
			reconcilingDraft = false;
		}
	}
	$effect(() => {
		const term = academicTermId;
		const userId = $authStore.user?.id;
		const allowed = $can.hasAny(
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL
		);
		if (!term || !userId || !allowed) return;
		connectTimetableSocket({ academicTermId: term, currentUserId: userId });
		return () => disconnectTimetableSocket();
	});
	onMount(() => {
		let initial = true;
		const unsubscribe = refreshTrigger.subscribe(() => {
			if (initial) {
				initial = false;
				return;
			}
			void reconcileOpenDraft();
		});
		const unregister = registerDeliveryDraftReconcile(
			reconcileOpenDraft,
			() =>
				!$can.hasAny(
					PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL
				)
		);
		return () => {
			unsubscribe();
			unregister();
		};
	});

	$effect.pre(() => {
		const routeResult = data.changeSetSummaries;
		const { revision } = changeSetSummaryRequest.begin();
		untrack(() => {
			changeSets = [];
			changeSetSummaryLoading = Boolean(routeResult);
			changeSetSummaryResolved = false;
			changeSetSummaryError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!changeSetSummaryRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						applyChangeSetSummaries(result.data);
						changeSetSummaryResolved = true;
					} else changeSetSummaryError = result.error;
					changeSetSummaryLoading = false;
				});
			});
		}
		return () => {
			if (changeSetSummaryRequest.isCurrent(revision)) changeSetSummaryRequest.abort();
		};
	});

	$effect.pre(() => {
		const routeResult = data.selectedChangeSet;
		const { revision } = changeSetRequest.begin();
		untrack(() => {
			activeChangeSet = null;
			changeSetLoading = Boolean(routeResult);
			changeSetError = '';
		});
		if (routeResult) {
			void routeResult.then((result) => {
				if (!changeSetRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						activeChangeSet = result.data;
						if (result.data) selectedChangeSetId = result.data.id;
					} else changeSetError = result.error;
					changeSetLoading = false;
				});
			});
		}
		return () => {
			if (changeSetRequest.isCurrent(revision)) changeSetRequest.abort();
		};
	});

	$effect.pre(() => {
		const termId = academicTermId;
		const selectedVersionId = workspace?.deliveryVersionId;
		overviewRequest.abort();
		untrack(() => {
			overview = null;
			revisionOfferings = [];
			overviewLoading = false;
			errorMessage = '';
			if (!termId) viewMode = 'homerooms';
			else if (selectedVersionId && viewMode === 'offerings') void loadOverview(termId);
		});
		return () => overviewRequest.abort();
	});
</script>

<PageShell
	title="จัดการการเปิดสอน"
	description="จัดรายวิชา กลุ่มเรียน ครู และจำนวนคาบ แล้วเผยแพร่รุ่นเปิดสอนเพื่อนำไปจัดตาราง"
>
	{#snippet actions()}
		{#if academicTermId}<Button
				variant="outline"
				disabled={homeroomLoading || changeSetSummaryLoading || deliveryVersionsLoading}
				onclick={() =>
					Promise.all([
						refreshDeliveryVersions(),
						refreshDeliveryRegions(),
						loadChangeSetSummaries()
					])}
			>
				<RefreshCw class="size-4" />โหลดล่าสุด
			</Button>{/if}
		{#if canManage && academicTermId}
			{#key academicTermId}
				<OfferingCreateDialog
					bind:this={offeringDialog}
					showTrigger={workspace?.deliveryVersionStatus === 'draft'}
					{academicTermId}
					onCreated={addCreated}
					onApplied={() => refreshDeliveryRegions(true)}
					defaultDeliveryVersionId={workspace?.deliveryVersionStatus === 'draft'
						? workspace.deliveryVersionId
						: null}
				/>
				<AcademicChangeSetDialog {academicTermId} onCreated={addChangeSet} />
				{#if canManage}
					<AcademicChangeSetDialog
						bind:this={deliveryRevisionDialog}
						{academicTermId}
						showTrigger={false}
						onCreated={handleDeliveryRevisionCreated}
						{sourceNotice}
						onCancelled={() => (pendingDeliveryAction = null)}
					/>
				{/if}
			{/key}
		{/if}
	{/snippet}

	{#if !academicYearId || !academicTermId}
		<AcademicPrerequisiteNotice prerequisite={missingTermPrerequisite} />
	{:else}
		<div class="space-y-4">
			{#if activationBusy}<p role="status" class="text-sm text-muted-foreground">
					กำลังเปิดกิจกรรมในแบบร่าง
				</p>{/if}
			{#if activationError}<div
					role="alert"
					class="rounded-xl border border-destructive/40 p-3 text-sm"
				>
					<p>{activationError}</p>
					<Button
						variant="outline"
						disabled={activationBusy}
						onclick={() => {
							if (pendingDeliveryAction)
								void prepareSynchronizedActivity(pendingDeliveryAction.catalogVersionId);
						}}>ลองอีกครั้ง</Button
					>
				</div>{/if}
			<section
				class="rounded-xl border bg-card p-3 sm:p-4"
				aria-label="รุ่นเปิดสอน"
				aria-busy={deliveryVersionsLoading}
			>
				{#if deliveryVersionsLoading && deliveryVersions.length === 0}<PageSkeleton
						variant="cards"
						rows={1}
					/>
				{:else if deliveryVersionsError}<PageState
						variant="error"
						title="โหลดรุ่นเปิดสอนไม่สำเร็จ"
						description={deliveryVersionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={refreshDeliveryVersions}
					/>
				{:else if deliveryVersions.length > 0}
					<div class="space-y-2">
						<p class="text-sm font-medium">รุ่นเปิดสอน</p>
						<Select.Root
							type="single"
							value={workspace?.deliveryVersionId ?? ''}
							onValueChange={selectDeliveryVersion}
							><Select.Trigger class="w-full" aria-label="เลือกรุ่นเปิดสอน"
								>{selectedDelivery
									? deliveryVersionLabel(selectedDelivery)
									: 'เลือกรุ่นเปิดสอน'}</Select.Trigger
							><Select.Content
								>{#each deliveryVersions as version (version.id)}<Select.Item value={version.id}
										>{deliveryVersionLabel(version)}</Select.Item
									>{/each}</Select.Content
							></Select.Root
						>
						<p class="text-xs text-muted-foreground">
							รุ่นเปิดสอนหนึ่งรุ่นใช้จัดตารางได้หลายรุ่น การเผยแพร่ที่หน้านี้ไม่เปลี่ยนตารางเดิม
						</p>
					</div>
				{/if}
			</section>
			<section
				class="relative space-y-4"
				aria-label="การจัดการรุ่นเปิดสอน"
				aria-busy={changeSetSummaryLoading || changeSetLoading}
				data-testid={changeSetSummaryResolved &&
				!changeSetSummaryLoading &&
				!changeSetLoading &&
				!changeSetSummaryError &&
				!changeSetError
					? 'delivery-change-set-ready'
					: undefined}
			>
				{#if changeSetSummaryLoading && changeSets.length === 0 && !activeChangeSet}
					<PageSkeleton variant="detail" />
				{:else if changeSetSummaryError && changeSets.length === 0 && !activeChangeSet}
					<PageState
						variant="error"
						title="โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ"
						description={changeSetSummaryError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadChangeSetSummaries}
					/>
				{:else}
					{#if (changeSetSummaryLoading || changeSetLoading) && activeChangeSet}
						<RegionUpdatingState label="กำลังอัปเดตชุดการเปลี่ยนแปลงกลางภาค" />
					{/if}
					{#if changeSetLoading && !activeChangeSet}
						<PageSkeleton variant="detail" />
					{:else if changeSetError && !activeChangeSet}
						<PageState
							variant="error"
							title="โหลดรายละเอียดชุดการเปลี่ยนแปลงไม่สำเร็จ"
							description={changeSetError}
							actionLabel="ลองอีกครั้ง"
							onaction={() => loadSelectedChangeSet(selectedChangeSetId, false)}
						/>
					{:else if activeChangeSet}
						{#key activeChangeSet.id}
							<AcademicChangeSetPanel
								changeSet={activeChangeSet}
								offerings={revisionOfferings}
								{canManage}
								ensureOfferings={ensureOverview}
								initialTeacherChangeItemId={page.url.searchParams.get('teacherChangeItemId') ?? ''}
								onChanged={updateChangeSet}
								onDeleted={handleDraftDeleted}
							/>
						{/key}
					{:else if canManage}
						<section
							class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-dashed border-amber-500/35 bg-amber-500/5 p-3 text-sm"
						>
							<div>
								<p class="font-medium text-amber-900">เริ่มต้นด้วยรุ่นเปิดสอน</p>
								<p class="text-xs text-muted-foreground">
									ใช้ปุ่ม “สร้างรุ่นเปิดสอน” ด้านบน แล้วกำหนดรายวิชา กลุ่ม ครู
									และจำนวนคาบก่อนเผยแพร่
								</p>
							</div>
						</section>
					{/if}
				{/if}
			</section>
			<Tabs.Root value={viewMode} onValueChange={changeViewMode}>
				<Tabs.List class="grid w-full grid-cols-2 sm:w-[430px]">
					<Tabs.Trigger value="homerooms">มุมมองรายห้อง</Tabs.Trigger>
					<Tabs.Trigger value="offerings">มุมมองรายวิชา/กิจกรรม</Tabs.Trigger>
				</Tabs.List>
				<Tabs.Content value="homerooms" class="mt-4">
					<section
						class="relative"
						aria-label="มุมมองรายห้อง"
						aria-busy={homeroomLoading}
						data-testid={workspace ? 'delivery-homerooms-ready' : undefined}
					>
						{#if homeroomLoading && workspace}
							<RegionUpdatingState label="กำลังอัปเดตภาพรวมรายห้อง" />
						{/if}
						{#if homeroomLoading && !workspace}
							<PageSkeleton variant="cards" rows={4} />
						{:else if homeroomError && !workspace}
							<PageState
								variant="error"
								title="โหลดภาพรวมรายห้องไม่สำเร็จ"
								description={homeroomError}
								actionLabel="ลองอีกครั้ง"
								onaction={loadHomerooms}
							/>
						{:else if workspace}
							<HomeroomDeliveryWorkspace
								{workspace}
								{canManage}
								{activationBusy}
								onPrepareSynchronizedActivity={prepareSynchronizedActivity}
							/>
						{/if}
					</section>
				</Tabs.Content>
				<Tabs.Content value="offerings" class="mt-4">
					<section
						class="relative"
						aria-label="มุมมองรายวิชาและกิจกรรม"
						aria-busy={overviewLoading}
						data-testid={overview ? 'delivery-offerings-ready' : undefined}
					>
						{#if overviewLoading && overview}
							<RegionUpdatingState label="กำลังอัปเดตรายการเปิดสอน" />
						{/if}
						{#if overviewLoading && !overview}
							<PageSkeleton variant="table" rows={6} />
						{:else if errorMessage && !overview}
							<PageState
								variant="error"
								title="โหลดรายการเปิดสอนไม่สำเร็จ"
								description={errorMessage}
								actionLabel="ลองอีกครั้ง"
								onaction={() => void ensureOverview()}
							/>
						{:else if items.length === 0}
							<AcademicPrerequisiteNotice prerequisite={noOfferingPrerequisite} />
						{:else}
							<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
								<div
									class="flex flex-wrap items-start justify-between gap-4 border-b bg-muted/25 p-4"
								>
									<div>
										<h2 class="font-semibold">รายการเปิดสอนของรุ่นที่เลือก</h2>
										<p class="mt-1 text-sm text-muted-foreground">
											ใช้มุมมองนี้เมื่อต้องจัดรายละเอียดของรายวิชาหรือกิจกรรมใดกิจกรรมหนึ่ง
										</p>
									</div>
									<p class="rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary">
										{items.length} รายการ
									</p>
								</div>
								{#if overview}<DeliveryVersionOverviewTable version={overview} {initialKind} />{/if}
							</section>
						{/if}
					</section>
				</Tabs.Content>
			</Tabs.Root>

			<p class="text-xs text-muted-foreground">
				ข้อมูลทั้งหมดอ้างอิงปีการศึกษาและภาคเรียนที่เลือกบนแถบด้านบน
				การเปลี่ยนบริบทจะโหลดโครงสร้างและการเปิดสอนของภาคเรียนนั้นใหม่
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={chooseDraftOpen}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>เลือกร่างที่จะเพิ่มกิจกรรม</Dialog.Title><Dialog.Description
				>{sourceNotice}</Dialog.Description
			></Dialog.Header
		>
		<div class="space-y-2">
			{#each draftChoices as draft (draft.id)}
				<Button
					class="h-auto w-full justify-start whitespace-normal text-start"
					variant="outline"
					disabled={activationBusy}
					onclick={() => continueActivityInDraft(draft.id)}
					>{changeSets.find((item) => item.id === draft.changeSetId)?.reason || 'แบบร่าง'} · {draft.offeringCount}
					รายการ</Button
				>
			{/each}
		</div>
		{#if activationError}<p role="alert" class="text-sm text-destructive">{activationError}</p>{/if}
	</Dialog.Content>
</Dialog.Root>
