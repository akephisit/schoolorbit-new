import { getContext, setContext } from 'svelte';

const context = Symbol('supervision observation writes');

function createObservationWrites() {
	let changedAt = -Infinity;
	let revision = 0;
	let pending = 0;
	let settled = Promise.resolve();
	let resolveSettled = () => {};

	return {
		guardRead(startedAt?: number) {
			const readRevision = revision;
			return () =>
				pending === 0 &&
				revision === readRevision &&
				(startedAt === undefined || changedAt < startedAt);
		},
		whenSettled() {
			return settled;
		},
		beginChange() {
			if (pending === 0) settled = new Promise<void>((resolve) => (resolveSettled = resolve));
			pending += 1;
			revision += 1;
			changedAt = performance.now();
			return () => {
				pending -= 1;
				revision += 1;
				changedAt = performance.now();
				if (pending === 0) resolveSettled();
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
