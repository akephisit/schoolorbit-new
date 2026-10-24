import type { FilePurpose } from '#lib/api/files.js';

export interface UploadScope {
	purpose: FilePurpose;
	resourceId?: string;
	/** The owning form/resource generation, independent of the persisted file ID. */
	owner: string;
}

type Upload = (file: File, purpose: FilePurpose, resourceId?: string) => Promise<{ id: string }>;

/** Browser-owned preparation. Only commit() is allowed to start an upload. */
export class PendingUpload {
	readonly file: File;
	readonly scope: Readonly<UploadScope>;
	private upload: Upload;
	private active = true;
	private uploadedId: string | undefined;
	private request: Promise<string> | undefined;

	constructor(file: File, scope: UploadScope, upload: Upload) {
		this.file = file;
		this.scope = Object.freeze({ ...scope });
		this.upload = upload;
	}

	commit(): Promise<string> {
		if (!this.active) return Promise.reject(new Error('รายการไฟล์นี้ถูกเปลี่ยนแล้ว'));
		if (this.uploadedId) return Promise.resolve(this.uploadedId);
		if (this.request) return this.request;
		this.request = this.upload(this.file, this.scope.purpose, this.scope.resourceId)
			.then(({ id }) => {
				if (!this.active) throw new Error('รายการไฟล์นี้ถูกเปลี่ยนแล้ว');
				this.uploadedId = id;
				return id;
			})
			.finally(() => {
				this.request = undefined;
			});
		return this.request;
	}

	dispose(): void {
		this.active = false;
		// A late/ambiguous response cannot prove that a remote file is unreferenced.
	}
}
