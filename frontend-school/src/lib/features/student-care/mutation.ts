/** Per-form idempotency identity: retry unchanged input with the same receipt key. */
export class CareMutationIdentity {
	private fingerprint = '';
	private requestId = '';
	prepare<T extends { requestId: string }>(payload: T, operation: string): T {
		const { requestId: _requestId, ...content } = payload;
		const fingerprint = JSON.stringify([operation, content]);
		if (fingerprint !== this.fingerprint) {
			this.fingerprint = fingerprint;
			this.requestId = crypto.randomUUID();
		}
		return { ...payload, requestId: this.requestId };
	}
	clear() {
		this.fingerprint = '';
		this.requestId = '';
	}
}
