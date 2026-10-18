/** Preserve a retry key for an identical payload; edits start a distinct mutation. */
export class LeaveMutationIntent {
	private basis = '';
	private id = '';
	for(payload: object): string {
		const basis = JSON.stringify(payload);
		if (!this.id || basis !== this.basis) {
			this.basis = basis;
			this.id = crypto.randomUUID();
		}
		return this.id;
	}
	clear() {
		this.basis = '';
		this.id = '';
	}
}
