export type GradebookSaveState = 'saved' | 'unsaved' | 'saving' | 'failed';

export interface GradebookSaveQueueSnapshot {
	state: GradebookSaveState;
	pendingCount: number;
	error: Error | null;
}

export interface GradebookSaveQueueOptions<TMutation> {
	delayMs?: number;
	keyOf: (mutation: TMutation) => string;
	partitionKey?: (mutation: TMutation) => string;
	saveBatch: (mutations: TMutation[]) => Promise<void>;
}

export interface GradebookSaveQueue<TMutation> {
	enqueue: (mutation: TMutation) => void;
	flush: () => Promise<void>;
	retry: () => Promise<void>;
	discard: () => void;
	status: () => GradebookSaveQueueSnapshot;
	subscribe: (listener: (snapshot: GradebookSaveQueueSnapshot) => void) => () => void;
}

const DEFAULT_DELAY_MS = 750;

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error('ไม่สามารถบันทึกคะแนนได้');
}

export function createGradebookSaveQueue<TMutation>(
	options: GradebookSaveQueueOptions<TMutation>
): GradebookSaveQueue<TMutation> {
	const delayMs = options.delayMs ?? DEFAULT_DELAY_MS;
	if (!Number.isFinite(delayMs) || delayMs < 0) {
		throw new RangeError('delayMs must be a non-negative finite number');
	}

	const pending = new Map<string, TMutation>();
	const listeners = new Set<(snapshot: GradebookSaveQueueSnapshot) => void>();
	let state: GradebookSaveState = 'saved';
	let error: Error | null = null;
	let timer: ReturnType<typeof setTimeout> | null = null;
	let inFlight: Promise<void> | null = null;
	let discardRevision = 0;

	const snapshot = (): GradebookSaveQueueSnapshot => ({
		state,
		pendingCount: pending.size,
		error
	});

	const notify = () => {
		const current = snapshot();
		for (const listener of listeners) listener(current);
	};

	const clearTimer = () => {
		if (timer === null) return;
		clearTimeout(timer);
		timer = null;
	};

	const schedule = () => {
		clearTimer();
		if (state === 'failed') return;
		timer = setTimeout(() => {
			timer = null;
			void flush().catch(() => undefined);
		}, delayMs);
	};

	const drain = async (): Promise<void> => {
		while (pending.size > 0) {
			state = 'saving';
			error = null;
			const revision = discardRevision;
			const pendingEntries = [...pending.entries()];
			const first = pendingEntries[0];
			const partition = first && options.partitionKey?.(first[1]);
			const entries = options.partitionKey
				? pendingEntries.filter(([, mutation]) => options.partitionKey?.(mutation) === partition)
				: pendingEntries;
			for (const [key, mutation] of entries) {
				if (pending.get(key) === mutation) pending.delete(key);
			}
			notify();

			try {
				await options.saveBatch(entries.map(([, mutation]) => mutation));
			} catch (reason) {
				if (revision !== discardRevision) continue;
				for (const [key, mutation] of entries) {
					if (!pending.has(key)) pending.set(key, mutation);
				}
				state = 'failed';
				error = asError(reason);
				notify();
				throw error;
			}
		}

		state = 'saved';
		error = null;
		notify();
	};

	const flush = (): Promise<void> => {
		clearTimer();
		if (inFlight) return inFlight;
		if (pending.size === 0) {
			state = 'saved';
			error = null;
			notify();
			return Promise.resolve();
		}

		const operation = drain();
		inFlight = operation;
		void operation
			.finally(() => {
				if (inFlight === operation) inFlight = null;
			})
			.catch(() => undefined);
		return operation;
	};

	return {
		enqueue(mutation) {
			const key = options.keyOf(mutation);
			if (!key) throw new Error('gradebook mutation key must not be empty');
			pending.set(key, mutation);
			if (state !== 'failed') state = inFlight ? 'saving' : 'unsaved';
			notify();
			schedule();
		},
		flush,
		async retry() {
			error = null;
			if (pending.size > 0) state = 'unsaved';
			notify();
			await flush();
		},
		discard() {
			clearTimer();
			discardRevision += 1;
			pending.clear();
			error = null;
			state = inFlight ? 'saving' : 'saved';
			notify();
		},
		status: snapshot,
		subscribe(listener) {
			listeners.add(listener);
			listener(snapshot());
			return () => listeners.delete(listener);
		}
	};
}
