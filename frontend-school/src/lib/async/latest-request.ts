export class LatestRequest {
	private controller: AbortController | undefined;
	private revision = 0;

	begin(): { revision: number; signal: AbortSignal } {
		this.controller?.abort();
		this.controller = new AbortController();
		this.revision += 1;
		return { revision: this.revision, signal: this.controller.signal };
	}

	isCurrent(revision: number): boolean {
		return revision === this.revision && this.controller?.signal.aborted === false;
	}

	abort(): void {
		this.controller?.abort();
		this.controller = undefined;
	}
}

export function isAbortError(error: unknown): boolean {
	return (
		(error instanceof DOMException && error.name === 'AbortError') ||
		(error instanceof Error && error.name === 'AbortError')
	);
}
