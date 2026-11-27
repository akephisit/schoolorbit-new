import { getContext, setContext } from 'svelte';
import { appIdentityKey } from '#lib/auth/settled-user.js';

const context = Symbol('supervision observation writes');

type ObservationScope = { academicYearId: string; academicTermId?: string | null };
type WriteSlot = {
	changedAt: number;
	revision: number;
	pending: number;
	settled: Promise<void>;
	resolveSettled: () => void;
};

function scopeKey(owner: string, { academicYearId, academicTermId }: ObservationScope) {
	return JSON.stringify([owner, academicYearId, academicTermId ?? null]);
}

function createObservationWrites() {
	const slots = new Map<string, WriteSlot>();

	return {
		guardRead(scope: ObservationScope & { academicTermId: string }, startedAt?: number) {
			const owner = appIdentityKey();
			const keys = [
				scopeKey(owner, scope),
				scopeKey(owner, { academicYearId: scope.academicYearId })
			];
			const watched = keys.map((key) => ({ key, revision: slots.get(key)?.revision ?? 0 }));
			return {
				isCurrent: () =>
					watched.every(({ key, revision }) => {
						const slot = slots.get(key);
						return (
							!slot ||
							(slot.pending === 0 &&
								slot.revision === revision &&
								(startedAt === undefined || slot.changedAt < startedAt))
						);
					}),
				whenSettled: () => Promise.all(keys.map((key) => slots.get(key)?.settled))
			};
		},
		beginChange(scope: ObservationScope) {
			const key = scopeKey(appIdentityKey(), scope);
			const slot = slots.get(key) ?? {
				changedAt: -Infinity,
				revision: 0,
				pending: 0,
				settled: Promise.resolve(),
				resolveSettled: () => {}
			};
			if (slot.pending === 0)
				slot.settled = new Promise<void>((resolve) => (slot.resolveSettled = resolve));
			slot.pending += 1;
			slot.revision += 1;
			slot.changedAt = performance.now();
			slots.set(key, slot);
			return () => {
				slot.pending -= 1;
				slot.revision += 1;
				slot.changedAt = performance.now();
				if (slot.pending === 0) slot.resolveSettled();
			};
		}
	};
}

export function provideSupervisionObservationWrites() {
	setContext(context, createObservationWrites());
}

export function useSupervisionObservationWrites() {
	return getContext<ReturnType<typeof createObservationWrites>>(context);
}
