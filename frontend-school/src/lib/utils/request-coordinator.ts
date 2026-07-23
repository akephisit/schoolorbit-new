export interface RequestCoordinator {
	run<T>(scope: string, key: string, operation: (signal: AbortSignal) => Promise<T>): Promise<T>;
	abort(scope: string): void;
	abortAll(): void;
}

interface InFlightRequest {
	key: string;
	controller: AbortController;
	promise: Promise<unknown>;
}

function abortReason(): DOMException {
	return new DOMException('Request aborted', 'AbortError');
}

export function isAbortError(error: unknown): boolean {
	return error instanceof DOMException && error.name === 'AbortError';
}

export function createRequestCoordinator(): RequestCoordinator {
	const inFlightByScope = new Map<string, InFlightRequest>();

	return {
		run<T>(scope: string, key: string, operation: (signal: AbortSignal) => Promise<T>): Promise<T> {
			const current = inFlightByScope.get(scope);
			if (current?.key === key) {
				return current.promise as Promise<T>;
			}
			if (current) {
				current.controller.abort(abortReason());
			}

			const controller = new AbortController();
			let operationPromise: Promise<T>;
			try {
				operationPromise = Promise.resolve(operation(controller.signal));
			} catch (error) {
				operationPromise = Promise.reject(error);
			}

			const trackedPromise = operationPromise.finally(() => {
				if (inFlightByScope.get(scope)?.promise === trackedPromise) {
					inFlightByScope.delete(scope);
				}
			});
			inFlightByScope.set(scope, { key, controller, promise: trackedPromise });
			return trackedPromise;
		},

		abort(scope: string): void {
			const current = inFlightByScope.get(scope);
			if (!current) return;
			inFlightByScope.delete(scope);
			current.controller.abort(abortReason());
		},

		abortAll(): void {
			const requests = [...inFlightByScope.values()];
			inFlightByScope.clear();
			for (const request of requests) {
				request.controller.abort(abortReason());
			}
		}
	};
}
