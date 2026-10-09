import type { AttendanceFace, AttendanceConfiguration } from '#lib/api/attendance.js';
import type * as FaceApi from '@vladmandic/face-api';
export const FACE_MODEL = 'face-api-1.7.15-recognition-128';
let engine: Promise<typeof FaceApi> | null = null;
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
		video: { width: { ideal: 640 }, height: { ideal: 480 }, facingMode: 'user' },
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
export async function readFace(video: HTMLVideoElement) {
	const api = await faceEngine();
	const faces = await api
		.detectAllFaces(video, new api.TinyFaceDetectorOptions({ inputSize: 320, scoreThreshold: 0.7 }))
		.withFaceLandmarks(true)
		.withFaceDescriptors();
	if (faces.length !== 1) return null;
	const face = faces[0];
	if (face.detection.box.width < 100 || face.detection.box.height < 100) return null;
	const points = face.landmarks.positions;
	const left = points[36].x,
		right = points[45].x;
	const yaw = (points[30].x - (left + right) / 2) / Math.max(1, right - left);
	return { values: Array.from(face.descriptor), yaw };
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
	const sorted = faces
		.map((f) => ({
			studentId: f.studentId,
			distance: Math.min(...f.descriptors.map((d) => distance(d.values, values)))
		}))
		.sort((a, b) => a.distance - b.distance);
	if (
		!sorted[0] ||
		sorted[0].distance > configuration.faceDistance ||
		(sorted[1] && sorted[1].distance - sorted[0].distance < configuration.faceMargin)
	)
		return null;
	return sorted[0].studentId;
}
export async function evidence(video: HTMLVideoElement) {
	const canvas = document.createElement('canvas');
	canvas.width = 640;
	canvas.height = 480;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('ถ่ายภาพไม่ได้');
	context.drawImage(video, 0, 0, 640, 480);
	const blob = await new Promise<Blob>((resolve, reject) =>
		canvas.toBlob(
			(value) => (value ? resolve(value) : reject(new Error('ถ่ายภาพไม่ได้'))),
			'image/jpeg',
			0.75
		)
	);
	return new File([blob], `attendance-${crypto.randomUUID()}.jpg`, { type: 'image/jpeg' });
}
