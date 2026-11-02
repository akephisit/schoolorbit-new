/** Rates count only bytes acknowledged by the receiving storage writer. */
export class TransferRate {
	private at: number | null = null;
	private bytes = 0;
	private value = 0;
	reset(now: number, bytes = 0) {
		this.at = now;
		this.bytes = bytes;
		this.value = 0;
	}
	sample(now: number, bytes: number): number {
		if (this.at === null || now < this.at || bytes < this.bytes) {
			this.reset(now, bytes);
			return 0;
		}
		const elapsed = now - this.at;
		if (elapsed >= 500) {
			this.value = (bytes - this.bytes) / 1_000_000 / (elapsed / 1000);
			this.at = now;
			this.bytes = bytes;
		}
		return this.value;
	}
}
