type Download = { file: File; name: string };
type Ports = {
	read(index: number): Promise<Download>;
	request(index: number, download: Download): void;
	progress(count: number): void;
	failure(error: unknown): void;
};
/** One awaited reader and a desired prefix replace one promise per completed file. */
export class AutomaticDownloadPump {
	private desired = 0;
	private next = 0;
	private stopped = false;
	private task: Promise<void> | null = null;
	private ports: Ports;
	constructor(ports: Ports) {
		this.ports = ports;
	}
	advance(completedCount: number) {
		if (this.stopped || completedCount <= this.desired) return;
		this.desired = completedCount;
		if (!this.task) this.task = this.run();
	}
	private async run() {
		try {
			while (!this.stopped && this.next < this.desired) {
				const index = this.next,
					download = await this.ports.read(index);
				if (this.stopped) return;
				this.ports.request(index, download);
				this.next++;
				this.ports.progress(this.next);
			}
		} catch (error) {
			if (!this.stopped) {
				this.stopped = true;
				this.ports.failure(error);
			}
		} finally {
			this.task = null;
		}
	}
	stop(): Promise<void> {
		this.stopped = true;
		return this.task ?? Promise.resolve();
	}
}
