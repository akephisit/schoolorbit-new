import type { AttendanceFace, AttendanceConfiguration } from '#lib/api/attendance.js';
import type * as FaceApi from '@vladmandic/face-api';
export const FACE_MODEL = 'face-api-1.7.15-recognition-128';
let engine: Promise<typeof FaceApi> | null = null;
let detectorInputSize = 416;
let quickFrames = 0;
export async function faceEngine() {
	if (!engine)
		engine = (async () => {
			const api = await import('@vladmandic/face-api');
			await Promise.all([
				api.nets.tinyFaceDetector.loadFromUri('/attendance-models'),
				api.nets.faceLandmark68TinyNet.loadFromUri('/attendance-models'),
				api.nets.faceRecognitionNet.loadFromUri('/attendance-models')
			]);
			return api;
		})();
	try {
		return await engine;
	} catch (e) {
		engine = null;
		throw e;
	}
}
export async function camera(video: HTMLVideoElement, signal?: AbortSignal) {
	if (!navigator.mediaDevices?.getUserMedia)
		throw new Error('เปิดหน้านี้ผ่าน HTTPS และอนุญาตเว็บแคม');
	const stream = await navigator.mediaDevices.getUserMedia({
		video: { width: { ideal: 1280 }, height: { ideal: 720 }, facingMode: 'user' },
		audio: false
	});
	const cleanup = () => stopCamera(stream, video);
	if (signal?.aborted) {
		cleanup();
		signal.throwIfAborted();
	}
	signal?.addEventListener('abort', cleanup, { once: true });
	try {
		video.srcObject = stream;
		await video.play();
		signal?.throwIfAborted();
		return stream;
	} catch (e) {
		cleanup();
		throw e;
	} finally {
		signal?.removeEventListener('abort', cleanup);
	}
}
export function stopCamera(stream: MediaStream | null, video?: HTMLVideoElement) {
	stream?.getTracks().forEach((t) => t.stop());
	if (video) video.srcObject = null;
}
export type FaceBox = { x: number; y: number; width: number; height: number };
export type CameraFace = { values: number[]; yaw: number; box: FaceBox };
export type CameraFrame = {
	frame: HTMLCanvasElement;
	capturedAt: string;
	faces: CameraFace[];
	detectedCount: number;
};
export async function readFaces(
	video: HTMLVideoElement,
	enrollment = false,
	signal?: AbortSignal
): Promise<CameraFrame> {
	const api = await faceEngine();
	const frame = document.createElement('canvas');
	const width = video.videoWidth,
		height = video.videoHeight;
	if (!width || !height) throw new Error('รอภาพจากกล้องก่อน');
	frame.width = Math.min(width, 1280);
	frame.height = Math.round((height * frame.width) / width);
	const context = frame.getContext('2d');
	if (!context) throw new Error('อ่านภาพจากกล้องไม่ได้');
	context.drawImage(video, 0, 0, frame.width, frame.height);
	const capturedAt = new Date().toISOString();
	signal?.throwIfAborted();
	const detectionStarted = performance.now();
	const detections = await api.detectAllFaces(
		frame,
		new api.TinyFaceDetectorOptions({
			inputSize: enrollment ? 320 : detectorInputSize,
			scoreThreshold: 0.7
		})
	);
	signal?.throwIfAborted();
	if (!enrollment) {
		const elapsed = performance.now() - detectionStarted;
		if (elapsed > 700) {
			detectorInputSize = Math.max(320, detectorInputSize - 96);
			quickFrames = 0;
		} else if (elapsed < 150 && ++quickFrames >= 8) {
			detectorInputSize = Math.min(512, detectorInputSize + 96);
			quickFrames = 0;
		} else if (elapsed >= 150) quickFrames = 0;
	}

	// Bound expensive landmark/descriptor work; enrollment always needs exactly one person.
	const eligible = (enrollment && detections.length !== 1 ? [] : detections)
		.filter(
			(face) =>
				face.box.width >= (enrollment ? 96 : 64) && face.box.height >= (enrollment ? 96 : 64)
		)
		.sort((a, b) => b.score - a.score)
		.slice(0, enrollment ? 1 : 4);
	const crops = await api.extractFaces(frame, eligible);
	const faces: CameraFace[] = [];
	for (let i = 0; i < crops.length; i++) {
		signal?.throwIfAborted();
		const landmarks = await api.nets.faceLandmark68TinyNet.detectLandmarks(crops[i]);
		if (Array.isArray(landmarks)) throw new Error('อ่านจุดใบหน้าไม่ได้');
		const aligned = await api.extractFaces(crops[i], [
			landmarks.align(undefined, { useDlibAlignment: true })
		]);
		const descriptor = await api.computeFaceDescriptor(aligned[0]);
		if (Array.isArray(descriptor)) throw new Error('อ่านใบหน้าไม่ได้');
		const points = landmarks.positions;
		const left = points[36].x,
			right = points[45].x;
		const box = eligible[i].box;
		faces.push({
			values: Array.from(descriptor),
			yaw: (points[30].x - (left + right) / 2) / Math.max(1, right - left),
			box: {
				x: box.x / frame.width,
				y: box.y / frame.height,
				width: box.width / frame.width,
				height: box.height / frame.height
			}
		});
	}
	return { frame, capturedAt, faces, detectedCount: detections.length };
}
export async function readFace(video: HTMLVideoElement, signal?: AbortSignal) {
	const capture = await readFaces(video, true, signal);
	return capture.faces[0] ?? null;
}
export function distance(a: number[], b: number[]) {
	if (
		a.length !== 128 ||
		b.length !== 128 ||
		a.some((x) => !Number.isFinite(x)) ||
		b.some((x) => !Number.isFinite(x))
	)
		return Infinity;
	return Math.sqrt(a.reduce((sum, x, i) => sum + (x - b[i]) ** 2, 0));
}
export function matchFace(
	faces: AttendanceFace[],
	values: number[],
	configuration: AttendanceConfiguration
) {
	let best = { studentId: '', distance: Infinity },
		next = Infinity;
	for (const face of faces) {
		let score = Infinity;
		for (const descriptor of face.descriptors)
			score = Math.min(score, distance(descriptor.values, values));
		if (score < best.distance) {
			next = best.distance;
			best = { studentId: face.studentId, distance: score };
		} else next = Math.min(next, score);
	}
	if (
		!best.studentId ||
		best.distance > configuration.faceDistance ||
		next - best.distance < configuration.faceMargin
	)
		return null;
	return best.studentId;
}
export async function evidence(source: HTMLVideoElement | HTMLCanvasElement, box?: FaceBox) {
	const canvas = document.createElement('canvas');
	const width = source instanceof HTMLVideoElement ? source.videoWidth : source.width;
	const height = source instanceof HTMLVideoElement ? source.videoHeight : source.height;
	const x = box ? Math.max(0, (box.x - box.width * 0.35) * width) : 0;
	const y = box ? Math.max(0, (box.y - box.height * 0.35) * height) : 0;
	const right = box ? Math.min(width, (box.x + box.width * 1.35) * width) : width;
	const bottom = box ? Math.min(height, (box.y + box.height * 1.35) * height) : height;
	if (right <= x || bottom <= y) throw new Error('ถ่ายภาพไม่ได้');
	canvas.width = Math.min(640, Math.round(right - x));
	canvas.height = Math.max(1, Math.round(((bottom - y) * canvas.width) / (right - x)));
	const context = canvas.getContext('2d');
	if (!context) throw new Error('ถ่ายภาพไม่ได้');
	context.drawImage(source, x, y, right - x, bottom - y, 0, 0, canvas.width, canvas.height);
	const blob = await new Promise<Blob>((resolve, reject) =>
		canvas.toBlob(
			(value) => (value ? resolve(value) : reject(new Error('ถ่ายภาพไม่ได้'))),
			'image/jpeg',
			0.8
		)
	);
	return new File([blob], `attendance-${crypto.randomUUID()}.jpg`, { type: 'image/jpeg' });
}
