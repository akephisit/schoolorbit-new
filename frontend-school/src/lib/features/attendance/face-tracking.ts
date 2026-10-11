import type { FaceBox } from './face-camera.js';

export type FaceObservation = { studentId: string; box: FaceBox };
type Track = { frames: number; time: number; box: FaceBox };

/** A bounded frame-local tracker. Missing, duplicated and discontinuous identities reset. */
export class FaceTracker {
	private tracks = new Map<string, Track>();
	clear() {
		this.tracks.clear();
	}
	observe(observations: FaceObservation[], now: number): Set<string> {
		const counts = new Map<string, number>();
		for (const face of observations)
			counts.set(face.studentId, (counts.get(face.studentId) ?? 0) + 1);
		const next = new Map<string, Track>();
		const ready = new Set<string>();
		for (const face of observations) {
			if (counts.get(face.studentId) !== 1) continue;
			const previous = this.tracks.get(face.studentId);
			const center = (box: FaceBox) => [box.x + box.width / 2, box.y + box.height / 2];
			const [x, y] = center(face.box);
			const [px, py] = previous ? center(previous.box) : [0, 0];
			const continuous =
				previous && now - previous.time <= 2000 && Math.hypot(x - px, y - py) <= 0.18;
			const frames = continuous ? previous.frames + 1 : 1;
			next.set(face.studentId, { frames, time: now, box: face.box });
			if (frames >= 3) ready.add(face.studentId);
		}
		this.tracks = next;
		return ready;
	}
}

export function enrollmentPose(yaw: number, sample: number): boolean {
	return sample === 0 ? Math.abs(yaw) < 0.08 : sample === 1 ? yaw > 0.06 : yaw < -0.06;
}
