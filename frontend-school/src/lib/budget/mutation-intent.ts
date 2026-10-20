/** A retry reuses its key only while the complete mutation payload is unchanged. */
export class BudgetMutationIntent {
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
