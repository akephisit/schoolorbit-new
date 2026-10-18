import { ApiClientError } from '#lib/api/client.js';

/** A failed transport may have committed. Replay its exact command before accepting another. */
export class MutationIntent<T extends { requestId: string }> {
	private payload: T | undefined;
	get unresolved(): boolean {
		return this.payload !== undefined;
	}
	begin(payload: Omit<T, 'requestId'>): T {
		this.payload ??= JSON.parse(
			JSON.stringify({ ...payload, requestId: crypto.randomUUID() })
		) as T;
		return this.payload;
	}
	accepted(): void {
		this.payload = undefined;
	}
	rejected(cause: unknown): void {
		if (
			cause instanceof ApiClientError &&
			cause.status >= 400 &&
			cause.status < 500 &&
			cause.status !== 408
		)
			this.payload = undefined;
	}
}
