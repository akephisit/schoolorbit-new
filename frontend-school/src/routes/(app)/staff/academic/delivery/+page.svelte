<script lang="ts">
	import {
		deliveryPublicationLabel,
		deliveryPublicationSummaryLabel,
		publicationDate,
		publicationTime
	} from '#lib/academic/publication-presentation.js';
	import type { PageData } from './$types';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { untrack, onMount, onDestroy, tick } from 'svelte';
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
		type DeliveryOfferingAddKind,
		type LearningDeliveryRefreshScope
	} from '#lib/academic/learning-delivery-page.js';
	import {
		buildCurriculumPreparationTarget,
		deliveryPreparationDraftCandidates,
		type CurriculumPreparationFocus,
		type CurriculumPreparationTarget
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
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '#lib/components/academic-workflow/index.js';
	import AcademicChangeSetDialog from '#lib/components/learning-delivery/AcademicChangeSetDialog.svelte';
	import AcademicChangeSetPanel from '#lib/components/learning-delivery/AcademicChangeSetPanel.svelte';
	import DeliveryMissingRequirements from '#lib/components/learning-delivery/DeliveryMissingRequirements.svelte';
	import { buildDeliveryAlignmentFacts } from '#lib/academic/homeroom-delivery.js';
	import DeliveryAlignmentSummary from '#lib/components/learning-delivery/DeliveryAlignmentSummary.svelte';
	import HomeroomDeliveryWorkspace from '#lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte';
	import OfferingCreateDialog from '#lib/components/learning-delivery/OfferingCreateDialog.svelte';
	import DeliveryOfferingAddActions from '#lib/components/learning-delivery/DeliveryOfferingAddActions.svelte';
	import DeliveryVersionOverviewTable from '#lib/components/learning-delivery/DeliveryVersionOverviewTable.svelte';
	import { Button } from '#lib/components/ui/button/index.js';

	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import {
		deliveryOverviewStateHref,
		deliveryOverviewReturnHref,
		deliveryOfferingHref,
		readDeliveryOverviewState,
		type DeliveryOverviewState
	} from '#lib/navigation/learning-delivery.js';

	let { data }: { data: PageData } = $props();
	const overviewUrl = $derived(new URL((page.shallow?.url ?? page.url).href));
	const viewState = $derived(readDeliveryOverviewState(overviewUrl));
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
	async function refreshDeliveryVersions(propagateFailure = false): Promise<void> {
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
			if (propagateFailure && !isAbortError(error)) throw error;
		} finally {
			if (deliveryVersionsRequest.isCurrent(revision)) deliveryVersionsLoading = false;
		}
	}
	function deliveryVersionLabel(version: DeliveryVersionSummary): string {
		return `${deliveryPublicationLabel(version)} · ${version.offeringCount} รายการ`;
	}
	function selectDeliveryVersion(id: string): void {
		cancelPreparation();
		const url = new URL(overviewUrl);
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
	const comparisonWorkspace = $derived(
		workspace &&
			workspace.academicYearId === academicYearId &&
			workspace.academicTermId === academicTermId &&
			(!overviewUrl.searchParams.get('deliveryVersionId') ||
				workspace.deliveryVersionId === overviewUrl.searchParams.get('deliveryVersionId'))
			? workspace
			: null
	);
	const alignmentFacts = $derived(
		comparisonWorkspace ? buildDeliveryAlignmentFacts(comparisonWorkspace) : null
	);
	const returnTo = $derived(workspace ? deliveryOverviewReturnHref(overviewUrl, workspace) : '');
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
	const alignmentError = $derived(
		homeroomError ||
			(workspace && !comparisonWorkspace
				? 'ข้อมูลเทียบหลักสูตรไม่ตรงกับปี ภาคเรียน หรือรุ่นที่เลือก'
				: '')
	);
	let changeSetSummaryError = $state('');
	let changeSetError = $state('');
	let errorMessage = $state('');
	const viewMode = $derived(viewState.view);
	let offeringDialog = $state<{
		openManual: (kind: DeliveryOfferingAddKind) => Promise<void>;
		openCurriculumPreparation: (
			target: CurriculumPreparationTarget,
			deliveryVersionId?: string | null
		) => Promise<void>;
	}>();
	let deliveryRevisionDialog = $state<{ openDialog: () => void }>();
	type PreparationOperation = CurriculumPreparationFocus & {
		revision: number;
		signal: AbortSignal;
		context: string;
		academicYearId: string;
		academicTermId: string;
		expectedVersionId: string | null;
	};
	const preparationRequest = new LatestRequest();
	let preparationDisposed = false;
	let pendingDeliveryAction = $state.raw<PreparationOperation | null>(null);
	let activationBusy = $state(false);
	let activationError = $state('');
	let draftChoices = $state<DeliveryVersionSummary[]>([]);
	let chooseDraftOpen = $state(false);
	let sourceNotice = $state('');
	let canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	const preparationContext = $derived(
		`${academicYearId}:${academicTermId}:${$authStore.user?.id ?? ''}`
	);
	let observedPreparationContext = '';
	function preparationOwned(operation: PreparationOperation): boolean {
		return (
			!preparationDisposed &&
			canManage &&
			pendingDeliveryAction === operation &&
			operation.context === preparationContext &&
			preparationRequest.isCurrent(operation.revision)
		);
	}
	function preparationCurrent(operation: PreparationOperation): boolean {
		return (
			preparationOwned(operation) &&
			operation.expectedVersionId === overviewUrl.searchParams.get('deliveryVersionId')
		);
	}
	function settlePreparation(operation: PreparationOperation) {
		if (!preparationOwned(operation)) return;
		if (operation.expectedVersionId !== overviewUrl.searchParams.get('deliveryVersionId'))
			cancelPreparation();
		else activationBusy = false;
	}

	function cancelPreparation() {
		preparationRequest.abort();
		pendingDeliveryAction = null;
		activationBusy = false;
		activationError = '';
		chooseDraftOpen = false;
		draftChoices = [];
		sourceNotice = '';
	}
	$effect.pre(() => {
		const context = preparationContext,
			allowed = canManage,
			versionId = overviewUrl.searchParams.get('deliveryVersionId');
		untrack(() => {
			if (
				context !== observedPreparationContext ||
				!allowed ||
				(pendingDeliveryAction && pendingDeliveryAction.expectedVersionId !== versionId)
			) {
				observedPreparationContext = context;
				cancelPreparation();
			}
		});
	});
	onDestroy(() => {
		preparationDisposed = true;
		preparationRequest.abort();
	});

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

	function updateSelectedChangeSetUrl(id: string) {
		const url = new URL(overviewUrl);

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
			overviewUrl.searchParams.get('changeSetId')?.trim() || selectedChangeSetId
		);
		selectedChangeSetId = selected?.id ?? '';
		if (!selected) activeChangeSet = null;
		return selected;
	}

	async function loadHomerooms(propagateFailure = false) {
		if (!academicYearId || !academicTermId) return;
		const { revision, signal } = homeroomRequest.begin();
		homeroomLoading = true;
		homeroomError = '';
		try {
			const result = await getHomeroomDeliveryWorkspace(academicYearId, academicTermId, {
				deliveryVersionId: overviewUrl.searchParams.get('deliveryVersionId') ?? undefined,
				signal
			});
			if (homeroomRequest.isCurrent(revision)) workspace = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (homeroomRequest.isCurrent(revision)) {
				homeroomError = error instanceof Error ? error.message : 'โหลดภาพรวมรายห้องไม่สำเร็จ';
				if (propagateFailure) throw error;
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
		const url = new URL(overviewUrl);
		url.searchParams.delete('changeSetId');
		if (sourceVersionId) url.searchParams.set('deliveryVersionId', sourceVersionId);
		else url.searchParams.delete('deliveryVersionId');
		await goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`), {
			replace: true
		});
		await Promise.all([refreshDeliveryVersions(), loadChangeSetSummaries(), loadHomerooms()]);
	}

	async function loadOverview(termId: string, propagateFailure = false) {
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
			if (propagateFailure) throw error;
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
		const view = value === 'offerings' ? 'offerings' : 'homerooms';
		updateViewState({ view });
		if (view === 'offerings' && academicTermId && !overview && !overviewLoading)
			void loadOverview(academicTermId);
	}
	function updateViewState(patch: Partial<DeliveryOverviewState>) {
		void goto(deliveryOverviewStateHref(overviewUrl, patch), {
			shallow: true,
			replace: true,
			reset: false,
			state: page.state
		});
	}

	function addCreated(_item: LearningOfferingOverviewItem) {
		void refreshDeliveryVersions();
		void refreshDeliveryRegions();
	}

	async function continuePreparationInDraft(versionId: string, operation = pendingDeliveryAction) {
		if (!operation || !preparationCurrent(operation)) return;
		activationBusy = true;
		activationError = '';
		chooseDraftOpen = false;
		try {
			await tick();
			if (!preparationCurrent(operation)) return;
			let selected = workspace;
			if (selected?.deliveryVersionId !== versionId) {
				const url = new URL(overviewUrl);
				url.searchParams.set('deliveryVersionId', versionId);
				url.searchParams.delete('changeSetId');
				const previousVersionId = operation.expectedVersionId;
				operation.expectedVersionId = versionId;
				try {
					await goto(resolve(`staff/academic/delivery?${url.searchParams.toString()}`));
				} catch (error) {
					if (
						preparationOwned(operation) &&
						overviewUrl.searchParams.get('deliveryVersionId') === previousVersionId
					)
						operation.expectedVersionId = previousVersionId;
					throw error;
				}
				if (!preparationCurrent(operation)) return;
				await tick();
				if (!preparationCurrent(operation)) return;
				const result = await data.homerooms;
				if (!preparationCurrent(operation)) return;
				if (!result?.ok) throw new Error(result?.error ?? 'โหลดแบบร่างไม่สำเร็จ');
				selected = result.data;
			}
			if (!preparationCurrent(operation)) return;
			if (
				!selected ||
				selected.academicYearId !== operation.academicYearId ||
				selected.academicTermId !== operation.academicTermId ||
				selected.deliveryVersionStatus !== 'draft' ||
				selected.deliveryVersionId !== versionId
			)
				throw new Error('แบบร่างนี้ไม่พร้อมแก้ไข กรุณาโหลดล่าสุดแล้วลองอีกครั้ง');
			const focus = {
				resourceKind: operation.resourceKind,
				catalogVersionId: operation.catalogVersionId
			};
			const target = buildCurriculumPreparationTarget(selected, focus);
			if (target) {
				const dialog = offeringDialog;
				if (!dialog) throw new Error('หน้าต่างเตรียมรายการยังไม่พร้อม กรุณาลองอีกครั้ง');
				await dialog.openCurriculumPreparation(target, versionId);
				if (!preparationCurrent(operation)) return;
			} else {
				const existing = selected.homerooms
					.flatMap((room) => room.items)
					.find(
						(item) =>
							item.catalogVersionId === operation.catalogVersionId &&
							item.resourceKind === operation.resourceKind &&
							item.offeringId
					);
				if (!existing?.offeringId)
					throw new Error('ไม่พบรายการฉบับนี้ในหลักสูตรของร่าง กรุณาตรวจรายการอีกครั้ง');
				await goto(
					deliveryOfferingHref(
						existing.offeringId,
						selected,
						deliveryOverviewReturnHref(overviewUrl, selected)
					)
				);
				if (!preparationCurrent(operation)) return;
			}
			activationBusy = false;
			pendingDeliveryAction = null;
			chooseDraftOpen = false;
		} catch (error) {
			if (preparationCurrent(operation) && !isAbortError(error))
				activationError = error instanceof Error ? error.message : 'เตรียมรายการในร่างไม่สำเร็จ';
		} finally {
			settlePreparation(operation);
		}
	}

	async function prepareCurriculumRequirement(focus: CurriculumPreparationFocus) {
		if (
			!workspace ||
			!comparisonWorkspace ||
			homeroomLoading ||
			Boolean(alignmentError) ||
			!canManage ||
			!academicYearId ||
			!academicTermId ||
			(activationBusy && pendingDeliveryAction && preparationCurrent(pendingDeliveryAction))
		)
			return;
		const ticket = preparationRequest.begin();
		const operation: PreparationOperation = {
			resourceKind: focus.resourceKind,
			catalogVersionId: focus.catalogVersionId,
			...ticket,
			context: preparationContext,
			academicYearId,
			academicTermId,
			expectedVersionId: overviewUrl.searchParams.get('deliveryVersionId')
		};
		const originVersionId = workspace.deliveryVersionId;
		pendingDeliveryAction = operation;
		activationBusy = true;
		activationError = '';
		try {
			if (workspace.deliveryVersionStatus === 'draft' && originVersionId) {
				await continuePreparationInDraft(originVersionId, operation);
				return;
			}
			const versions = await listDeliveryVersions(operation.academicTermId, {
				signal: operation.signal
			});
			if (!preparationCurrent(operation)) return;
			const { source, drafts } = deliveryPreparationDraftCandidates(versions);
			if (!source) throw new Error('ไม่พบรุ่นเปิดสอนที่เผยแพร่แล้ว กรุณาสร้างรุ่นเปิดสอนก่อน');
			deliveryVersions = versions;
			sourceNotice =
				originVersionId !== source.id
					? 'กำลังดูรุ่นย้อนหลัง ร่างสำหรับเปิดรายการตามหลักสูตรจะใช้ข้อมูลจากรุ่นเผยแพร่ล่าสุด'
					: 'เปิดรายการตามหลักสูตรในร่างจากรุ่นเผยแพร่ล่าสุด รุ่นที่เผยแพร่แล้วจะคงข้อมูลเดิม';
			draftChoices = drafts;
			if (drafts.length === 1) {
				if (originVersionId !== source.id) toast.info(sourceNotice);
				await continuePreparationInDraft(drafts[0].id, operation);
				if (!preparationCurrent(operation)) return;
			} else if (drafts.length > 1) chooseDraftOpen = true;
			else if (deliveryRevisionDialog) deliveryRevisionDialog.openDialog();
			else throw new Error('หน้าต่างสร้างร่างยังไม่พร้อม กรุณาลองอีกครั้ง');
		} catch (error) {
			if (preparationCurrent(operation) && !isAbortError(error))
				activationError = error instanceof Error ? error.message : 'ค้นหาแบบร่างไม่สำเร็จ';
		} finally {
			settlePreparation(operation);
		}
	}

	async function handleDeliveryRevisionCreated(created: AcademicTermChangeSet) {
		const operation = pendingDeliveryAction;
		if (created.academicTermId !== academicTermId) return;
		if (operation && preparationCurrent(operation))
			await continuePreparationInDraft(created.targetDeliveryVersionId, operation);
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

	async function draftResourceRemoved(updated: AcademicTermChangeSet) {
		if (
			updated.id !== activeChangeSet?.id ||
			updated.targetDeliveryVersionId !== workspace?.deliveryVersionId
		)
			return;
		activeChangeSet = updated;
		changeSets = changeSets.map((item) =>
			item.id === updated.id ? summarizeAcademicTermChangeSet(updated) : item
		);
		await Promise.all([
			loadHomerooms(true),
			refreshDeliveryVersions(true),
			...(academicTermId && overview ? [loadOverview(academicTermId, true)] : [])
		]);
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
			if (termId && selectedVersionId && viewMode === 'offerings') void loadOverview(termId);
		});
		return () => overviewRequest.abort();
	});
</script>

<PageShell
	title="จัดการการเปิดสอน"
	description="จัดรายวิชา กลุ่มเรียน ครู และจำนวนคาบ แล้วเผยแพร่รุ่นเปิดสอนเพื่อนำไปจัดตาราง"
>
	{#snippet actions()}
		{#if academicTermId}
			<div
				class="w-full min-w-0 sm:w-80"
				aria-busy={deliveryVersionsLoading}
				data-testid="delivery-version-selector"
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
					<Select.Root
						type="single"
						value={workspace?.deliveryVersionId ?? ''}
						onValueChange={selectDeliveryVersion}
					>
						<Select.Trigger class="w-full min-w-0" aria-label="เลือกรุ่นเปิดสอน">
							<span class="min-w-0 truncate">
								{selectedDelivery
									? deliveryPublicationSummaryLabel(selectedDelivery)
									: 'เลือกรุ่นเปิดสอน'}
							</span>
						</Select.Trigger>
						<Select.Content class="w-(--bits-select-anchor-width) max-w-[calc(100vw-2rem)]">
							{#each deliveryVersions as version (version.id)}
								<Select.Item value={version.id} label={deliveryVersionLabel(version)}>
									<div class="min-w-0 space-y-1 whitespace-normal wrap-break-word">
										<div>{deliveryPublicationSummaryLabel(version)}</div>
										<div class="text-xs text-muted-foreground">
											{version.effectiveFrom
												? `เริ่มใช้ ${publicationDate(version.effectiveFrom)}`
												: 'เลือกวันเริ่มใช้ตอนเผยแพร่'} · {version.offeringCount} รายการ
										</div>
										{#if version.publishedAt}
											<div class="text-xs text-muted-foreground">
												เผยแพร่ {publicationTime(version.publishedAt)}
											</div>
										{/if}
									</div>
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
			</div>
		{/if}

		{#if canManage && academicTermId}
			{#key academicTermId}
				{#key overviewUrl.searchParams.get('deliveryVersionId')}
					<OfferingCreateDialog
						bind:this={offeringDialog}
						showTrigger={workspace?.deliveryVersionStatus === 'draft'}
						{academicTermId}
						{returnTo}
						onCreated={addCreated}
						onApplied={() => refreshDeliveryRegions(true)}
						defaultDeliveryVersionId={workspace?.deliveryVersionStatus === 'draft'
							? workspace.deliveryVersionId
							: null}
					/>
				{/key}
				<AcademicChangeSetDialog {academicTermId} onCreated={addChangeSet} />
				{#if canManage}
					<AcademicChangeSetDialog
						bind:this={deliveryRevisionDialog}
						{academicTermId}
						showTrigger={false}
						onCreated={handleDeliveryRevisionCreated}
						{sourceNotice}
						onCancelled={cancelPreparation}
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
					กำลังเตรียมรายการในแบบร่าง
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
							if (pendingDeliveryAction) void prepareCurriculumRequirement(pendingDeliveryAction);
						}}>ลองอีกครั้ง</Button
					>
				</div>{/if}
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
								{returnTo}
								ensureOfferings={ensureOverview}
								initialTeacherChangeItemId={overviewUrl.searchParams.get('teacherChangeItemId') ??
									''}
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
			<DeliveryAlignmentSummary
				workspace={comparisonWorkspace}
				facts={alignmentFacts}
				loading={homeroomLoading}
				error={alignmentError}
				{canManage}
				preparing={activationBusy}
				{returnTo}
				onRetry={() => loadHomerooms()}
				onPrepare={prepareCurriculumRequirement}
			/>
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
								filters={viewState}
								{returnTo}
								onFiltersChange={updateViewState}
								onPrepareCurriculumRequirement={comparisonWorkspace &&
								!alignmentError &&
								!homeroomLoading
									? (focus) => {
											void prepareCurriculumRequirement(focus);
										}
									: undefined}
								draftChangeSetId={canManage &&
								activeChangeSet?.status === 'draft' &&
								activeChangeSet.targetDeliveryVersionId === workspace.deliveryVersionId
									? activeChangeSet.id
									: null}
								onDraftResourceRemoved={draftResourceRemoved}
								onAddOffering={(kind) => {
									void offeringDialog?.openManual(kind);
								}}
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
						{#if canManage && workspace?.deliveryVersionStatus === 'draft'}<div class="mb-3">
								<DeliveryOfferingAddActions
									onAdd={(kind) => {
										void offeringDialog?.openManual(kind);
									}}
								/>
							</div>{/if}
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
								{#if overview}<DeliveryVersionOverviewTable
										draftChangeSetId={canManage &&
										activeChangeSet?.status === 'draft' &&
										activeChangeSet.targetDeliveryVersionId === overview.id
											? activeChangeSet.id
											: null}
										onDraftResourceRemoved={draftResourceRemoved}
										version={overview}
										comparisonWorkspace={overview.id === comparisonWorkspace?.deliveryVersionId
											? comparisonWorkspace
											: null}
										alignmentFacts={overview.id === comparisonWorkspace?.deliveryVersionId
											? alignmentFacts
											: null}
										filters={viewState}
										{returnTo}
										onFiltersChange={updateViewState}
									/>{/if}
							</section>
						{/if}
					</section>
					<DeliveryMissingRequirements
						workspace={comparisonWorkspace}
						facts={alignmentFacts}
						loading={homeroomLoading}
						error={alignmentError}
						{canManage}
						preparing={activationBusy}
						filters={viewState}
						showFilters={items.length === 0}
						onFiltersChange={updateViewState}
						onRetry={() => loadHomerooms()}
						onPrepare={prepareCurriculumRequirement}
					/>
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
			><Dialog.Title
				>{pendingDeliveryAction?.resourceKind === 'course'
					? 'เลือกร่างที่จะเพิ่มรายวิชา'
					: 'เลือกร่างที่จะเพิ่มกิจกรรม'}</Dialog.Title
			><Dialog.Description>{sourceNotice}</Dialog.Description></Dialog.Header
		>
		<div class="space-y-2">
			{#each draftChoices as draft (draft.id)}
				<Button
					class="h-auto w-full justify-start whitespace-normal text-start"
					variant="outline"
					disabled={activationBusy}
					onclick={() => continuePreparationInDraft(draft.id)}
					>{changeSets.find((item) => item.id === draft.changeSetId)?.reason || 'แบบร่าง'} · {draft.offeringCount}
					รายการ</Button
				>
			{/each}
		</div>
		{#if activationError}<p role="alert" class="text-sm text-destructive">{activationError}</p>{/if}
	</Dialog.Content>
</Dialog.Root>
