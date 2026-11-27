import { getContext, setContext } from 'svelte';
import type { SupervisionObservation } from '#lib/api/supervision.js';

const context = Symbol('supervision personal return');
const maxAgeMs = 60_000;

function createPersonalReturn() {
	let saved: { scope: string; items: SupervisionObservation[]; readAt: number } | null = null;
	let revision = 0;
	let changedAt = -Infinity;
	let pendingChanges = 0;
	let settled = Promise.resolve();
	let settleChanges = () => {};

	function clear() {
		saved = null;
		revision += 1;
		changedAt = performance.now();
	}

	function isCurrentRead(readRevision: number, startedAt?: number) {
		return (
			pendingChanges === 0 &&
			readRevision === revision &&
			(startedAt === undefined || changedAt < startedAt)
		);
	}

	return {
		clear,
		read(scope: string) {
			if (!saved || pendingChanges || saved.scope !== scope || Date.now() - saved.readAt > maxAgeMs)
				return null;
			return saved.items;
		},
		captureRead() {
			return revision;
		},
		isCurrentRead,
		whenSettled() {
			return settled;
		},
		remember(scope: string, userId: string, items: SupervisionObservation[], readRevision: number) {
			if (!isCurrentRead(readRevision)) return;
			saved = {
				scope,
				items: items.filter((item) => item.observedUserId === userId),
				readAt: Date.now()
			};
		},
		beginChange() {
			if (pendingChanges === 0) settled = new Promise<void>((resolve) => (settleChanges = resolve));
			pendingChanges += 1;
			clear();
			return () => {
				pendingChanges -= 1;
				clear();
				if (pendingChanges === 0) settleChanges();
			};
		}
	};
}

export function providePersonalSupervisionReturn() {
	return setContext(context, createPersonalReturn());
}

export function usePersonalSupervisionReturn() {
	return getContext<ReturnType<typeof createPersonalReturn>>(context);
}
