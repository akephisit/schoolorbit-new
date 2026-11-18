import {
	approveSupervisionObservationRequest,
	getSupervisionEvaluatorAvailability,
	replaceSupervisionObservationEvaluators,
	returnSupervisionObservationRequest,
	type SupervisionEvaluatorAvailability,
	type SupervisionEvaluatorAvailabilityResponse,
	type SupervisionObservation
} from '#lib/api/supervision.js';
import { requireApiData } from '#lib/api/client.js';
import { isAbortError, LatestRequest } from '#lib/async/latest-request.js';
import { toast } from 'svelte-sonner';

type RequestReviewAction = 'approve-request' | 'return-request' | 'save-evaluators';

type RequestReviewDraft = {
	evaluatorIds?: string[];
	comment?: string;
	availability?: SupervisionEvaluatorAvailabilityResponse;
	availabilityBasis?: string;
	requestBasis?: string;
	loading?: boolean;
	error?: string;
};

const actionMessages = {
	'approve-request': {
		failure: 'อนุมัติคำขอไม่สำเร็จ',
		success: 'อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว'
	},
	'return-request': { failure: 'ส่งกลับคำขอไม่สำเร็จ', success: 'ส่งกลับคำขอแล้ว' },
	'save-evaluators': { failure: 'แก้ไขผู้ประเมินไม่สำเร็จ', success: 'แก้ไขผู้ประเมินแล้ว' }
} satisfies Record<RequestReviewAction, { failure: string; success: string }>;

function availabilityBasis(observation: SupervisionObservation) {
	return JSON.stringify([
		observation.cycleId,
		observation.periods.map(({ id, observedAt, endsAt }) => [id, observedAt, endsAt])
	]);
}

export function createSupervisionRequestReview({
	getObservation,
	hasManagePermission,
	captureOwner,
	isBusy,
	getSavingAction,
	setSavingAction,
	onApplied,
	allowSaveEvaluators = false
}: {
	getObservation: (id: string) => SupervisionObservation | null | undefined;
	hasManagePermission: () => boolean;
	captureOwner: () => () => boolean;
	isBusy: () => boolean;
	getSavingAction: () => string | null;
	setSavingAction: (action: string | null) => void;
	onApplied: (observation: SupervisionObservation) => void;
	allowSaveEvaluators?: boolean;
}) {
	let drafts = $state<Record<string, RequestReviewDraft>>({});
	const availabilityRequests = new Map<string, LatestRequest>();

	function canReview(observation: SupervisionObservation) {
		return hasManagePermission() && observation.canManage && observation.status === 'requested';
	}

	function canChange(observation: SupervisionObservation) {
		return (
			canReview(observation) && !observation.evaluators.some(({ status }) => status === 'submitted')
		);
	}

	function draft(id: string) {
		drafts[id] ??= {};
		return drafts[id];
	}

	function selectedEvaluators(observation: SupervisionObservation) {
		const state = drafts[observation.id];
		const saved = new Map(observation.evaluators.map((item) => [item.evaluatorUserId, item]));
		const options = new Map(state?.availability?.items.map((item) => [item.id, item]));
		const ids = state?.evaluatorIds ?? [...saved.keys()];
		const availabilityIsCurrent = state?.availabilityBasis === availabilityBasis(observation);
		return ids.map((id) => {
			const evaluator = saved.get(id);
			const option = options.get(id);
			return {
				id,
				name: evaluator?.evaluatorDisplayName?.trim() || option?.name || 'ไม่ทราบชื่อผู้ประเมิน',
				blocked: availabilityIsCurrent && !option?.available,
				isRequired: evaluator?.isRequired ?? true,
				roleLabel: evaluator?.roleLabel
			};
		});
	}

	function reviewState(observation: SupervisionObservation) {
		const state = drafts[observation.id];
		if (!state) return;
		const basis = availabilityBasis(observation);
		return {
			comment: state.comment,
			availability: state.availabilityBasis === basis ? state.availability : undefined,
			loading: state.requestBasis === basis && state.loading,
			error: state.requestBasis === basis ? state.error : undefined,
			needsRecheck: Boolean(state.requestBasis && state.requestBasis !== basis)
		};
	}

	function canSubmit(observation: SupervisionObservation, action: RequestReviewAction) {
		if (!canChange(observation) || isBusy()) return false;
		if (action === 'return-request') return true;
		const state = reviewState(observation);
		if (state?.loading || state?.error || state?.needsRecheck) return false;
		const availability = state?.availability;
		if (action === 'save-evaluators' && (!allowSaveEvaluators || !availability)) return false;
		if (
			action === 'approve-request' &&
			availability &&
			(!availability.withinCycleDates || availability.overlap.blocked)
		)
			return false;
		const selected = selectedEvaluators(observation);
		return selected.some((item) => item.isRequired) && !selected.some((item) => item.blocked);
	}

	async function loadAvailability(id: string) {
		const observation = getObservation(id);
		if (!observation || !canReview(observation)) return;
		const basis = availabilityBasis(observation);
		if (drafts[id]?.requestBasis === basis && drafts[id]?.loading) return;
		const state = draft(id);
		const request = availabilityRequests.get(id) ?? new LatestRequest();
		availabilityRequests.set(id, request);
		const { revision, signal } = request.begin();
		const ownsRoute = captureOwner();
		const ownsRead = () => ownsRoute() && request.isCurrent(revision);
		state.requestBasis = basis;
		state.loading = true;
		state.error = '';
		try {
			const result = await getSupervisionEvaluatorAvailability(id, { signal });
			if (!ownsRead()) return;
			const current = getObservation(id);
			if (!current || !canReview(current) || availabilityBasis(current) !== basis) return;
			state.availability = result;
			state.availabilityBasis = basis;
			if (state.evaluatorIds) {
				const keep = new Set([
					...current.evaluators.map((item) => item.evaluatorUserId),
					...result.items.filter((item) => item.available).map((item) => item.id)
				]);
				state.evaluatorIds = state.evaluatorIds.filter((selectedId) => keep.has(selectedId));
			}
		} catch (error) {
			if (ownsRead() && !isAbortError(error)) {
				state.error =
					error instanceof Error ? error.message : 'ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้';
			}
		} finally {
			if (ownsRead()) state.loading = false;
		}
	}

	function editSelection(id: string, evaluatorId: string, toggle = false) {
		const observation = getObservation(id);
		if (!observation || !canChange(observation) || isBusy()) return;
		const selected = selectedEvaluators(observation).map((item) => item.id);
		if (selected.includes(evaluatorId)) {
			draft(id).evaluatorIds = selected.filter((item) => item !== evaluatorId);
		} else if (toggle) {
			const state = reviewState(observation);
			if (
				!state?.loading &&
				!state?.error &&
				state?.availability?.items.some((item) => item.id === evaluatorId && item.available)
			) {
				draft(id).evaluatorIds = [...selected, evaluatorId];
			}
		}
	}

	function setComment(id: string, value: string) {
		const observation = getObservation(id);
		if (observation && canChange(observation) && !isBusy()) draft(id).comment = value;
	}

	function cancelAvailability(id: string) {
		availabilityRequests.get(id)?.abort();
		availabilityRequests.delete(id);
		if (drafts[id]) drafts[id].loading = false;
	}

	function clear(id: string) {
		cancelAvailability(id);
		delete drafts[id];
	}

	function reset() {
		for (const request of availabilityRequests.values()) request.abort();
		availabilityRequests.clear();
		drafts = {};
	}

	async function submit(id: string, action: RequestReviewAction) {
		const observation = getObservation(id);
		if (!observation || !canSubmit(observation, action)) return;
		const ownsMutation = captureOwner();
		const messages = actionMessages[action];
		setSavingAction(`${action}:${id}`);
		try {
			const evaluators = selectedEvaluators(observation).map(({ id, isRequired, roleLabel }) => ({
				evaluatorUserId: id,
				isRequired,
				...(roleLabel == null ? {} : { roleLabel })
			}));
			const response = await (action === 'return-request'
				? returnSupervisionObservationRequest(id, { comment: drafts[id]?.comment || null })
				: action === 'approve-request'
					? approveSupervisionObservationRequest(id, { evaluators })
					: replaceSupervisionObservationEvaluators(id, { evaluators }));
			if (!ownsMutation()) return;
			const updated = requireApiData(response, messages.failure);
			if (action === 'save-evaluators') draft(id).evaluatorIds = undefined;
			else clear(id);
			onApplied(updated);
			toast.success(messages.success);
		} catch (error) {
			if (!ownsMutation()) return;
			if (action !== 'return-request') void loadAvailability(id);
			toast.error(error instanceof Error ? error.message : messages.failure);
		} finally {
			if (ownsMutation()) setSavingAction(null);
		}
	}

	return {
		canReview,
		canChange,
		canSubmit,
		isBusy,
		isSaving: (id: string, action: RequestReviewAction) => getSavingAction() === `${action}:${id}`,
		state: reviewState,
		selectedEvaluators,
		loadAvailability,
		cancelAvailability,
		toggleEvaluator: (id: string, evaluator: SupervisionEvaluatorAvailability) =>
			editSelection(id, evaluator.id, true),
		removeEvaluator: (id: string, evaluatorId: string) => editSelection(id, evaluatorId),
		setComment,
		reset,
		submit
	};
}

export type SupervisionRequestReview = ReturnType<typeof createSupervisionRequestReview>;
