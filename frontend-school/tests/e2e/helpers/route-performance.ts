import type { Page, Request, Response } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';

const inventory = JSON.parse(
	readFileSync(new URL('../../fixtures/route-data-loading-inventory.json', import.meta.url), 'utf8')
) as { routes: Array<{ route: string }> };
const inventoryRoutes = new Set(inventory.routes.map((record) => record.route));

export type RouteRegionSample = {
	route: string;
	region: string;
	navigationStartedAt: number;
	requestStartedAt: number;
	requestFinishedAt: number;
	firstUsefulPaintAt: number;
	responseBytes: number | null;
	decodedBytes: number | null;
	serverTiming: Record<string, number>;
};

export type RouteRegionSummary = {
	route: string;
	region: string;
	navigationToRequestMs: number;
	requestDurationMs: number;
	firstUsefulRegionMs: number;
	responseBytes: number | null;
	decodedBytes: number | null;
	serverTiming: Record<string, number>;
};

type RegionDefinition = {
	region: string;
	apiPath: string;
	readyTestId: string;
};

type CapturedRequest = {
	request: Request;
	startedAt: number;
	finishedAt: number | null;
	response: Response | null;
};

function metricNameAndDuration(part: string): [string, number] | null {
	const match = /^\s*([a-z][a-z0-9_]*)\s*;\s*dur=([0-9]+(?:\.[0-9]+)?)\s*$/i.exec(part);
	if (!match) return null;
	const duration = Number(match[2]);
	return Number.isFinite(duration) ? [match[1].toLowerCase(), duration] : null;
}

export function parseServerTiming(header: string | undefined): Record<string, number> {
	const metrics: Record<string, number> = {};
	for (const part of (header ?? '').split(',').slice(0, 20)) {
		const parsed = metricNameAndDuration(part);
		if (parsed) metrics[parsed[0]] = parsed[1];
	}
	return metrics;
}

export class RoutePerformanceProbe {
	private navigationStartedAt = 0;
	private readonly definitions = new Map<string, RegionDefinition>();
	private readonly paths = new Map<string, string>();
	private readonly captured = new Map<string, CapturedRequest>();

	constructor(
		private readonly page: Page,
		private readonly route: string,
		definitions: RegionDefinition[]
	) {
		if (!inventoryRoutes.has(route)) {
			throw new Error('Performance route must be an inventoried route template');
		}
		for (const definition of definitions) {
			if (this.definitions.has(definition.region) || this.paths.has(definition.apiPath)) {
				throw new Error('Performance probe regions and API paths must be unique');
			}
			this.definitions.set(definition.region, definition);
			this.paths.set(definition.apiPath, definition.region);
		}
		page.on('request', this.onRequest);
		page.on('response', this.onResponse);
		page.on('requestfinished', this.onRequestFinished);
	}

	private readonly onRequest = (request: Request) => {
		const region = this.paths.get(new URL(request.url()).pathname);
		if (!region || this.navigationStartedAt === 0) return;
		this.captured.set(region, {
			request,
			startedAt: performance.now(),
			finishedAt: null,
			response: null
		});
	};

	private readonly onResponse = (response: Response) => {
		const region = this.paths.get(new URL(response.url()).pathname);
		const captured = region ? this.captured.get(region) : null;
		if (captured?.request === response.request()) captured.response = response;
	};

	private readonly onRequestFinished = (request: Request) => {
		const region = this.paths.get(new URL(request.url()).pathname);
		const captured = region ? this.captured.get(region) : null;
		if (captured?.request === request) captured.finishedAt = performance.now();
	};

	beginNavigation(): void {
		this.captured.clear();
		this.navigationStartedAt = performance.now();
	}

	async sample(region: string): Promise<RouteRegionSample> {
		const definition = this.definitions.get(region);
		if (!definition || this.navigationStartedAt === 0) {
			throw new Error('Begin a navigation with a registered region before sampling');
		}
		await this.page.getByTestId(definition.readyTestId).waitFor({ state: 'visible' });
		await this.page.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
				)
		);
		const firstUsefulPaintAt = performance.now();
		const captured = this.captured.get(region);
		if (!captured?.response) throw new Error(`No completed request was captured for ${region}`);
		const responseError = await captured.response.finished();
		if (responseError) throw responseError;
		const sizes = await captured.request.sizes();
		const decodedBytes = await this.page.evaluate((apiPath) => {
			const entries = performance.getEntriesByType('resource') as PerformanceResourceTiming[];
			const matched = entries.filter((entry) => new URL(entry.name).pathname === apiPath);
			const size = matched.at(-1)?.decodedBodySize ?? 0;
			return size > 0 ? size : null;
		}, definition.apiPath);
		return {
			route: this.route,
			region,
			navigationStartedAt: this.navigationStartedAt,
			requestStartedAt: captured.startedAt,
			requestFinishedAt: captured.finishedAt ?? performance.now(),
			firstUsefulPaintAt,
			responseBytes: sizes.responseBodySize + sizes.responseHeadersSize || null,
			decodedBytes,
			serverTiming: parseServerTiming(captured.response.headers()['server-timing'])
		};
	}

	dispose(): void {
		this.page.off('request', this.onRequest);
		this.page.off('response', this.onResponse);
		this.page.off('requestfinished', this.onRequestFinished);
	}
}

function median(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.floor(sorted.length / 2)];
}

function roundedMedian(values: number[]): number {
	return Math.round(median(values) * 10) / 10;
}

function optionalMedian(values: Array<number | null>): number | null {
	const available = values.filter((value): value is number => value !== null);
	return available.length === values.length ? roundedMedian(available) : null;
}

export function summarizeFiveWarmRuns(samples: RouteRegionSample[]): RouteRegionSummary {
	if (samples.length !== 5) throw new Error('Exactly five warm route samples are required');
	const [{ route, region }] = samples;
	if (samples.some((sample) => sample.route !== route || sample.region !== region)) {
		throw new Error('Warm route samples must have the same route and region');
	}
	const metricNames = Object.keys(samples[0].serverTiming).filter((name) =>
		samples.every((sample) => Object.hasOwn(sample.serverTiming, name))
	);
	return {
		route,
		region,
		navigationToRequestMs: roundedMedian(
			samples.map((sample) => sample.requestStartedAt - sample.navigationStartedAt)
		),
		requestDurationMs: roundedMedian(
			samples.map((sample) => sample.requestFinishedAt - sample.requestStartedAt)
		),
		firstUsefulRegionMs: roundedMedian(
			samples.map((sample) => sample.firstUsefulPaintAt - sample.navigationStartedAt)
		),
		responseBytes: optionalMedian(samples.map((sample) => sample.responseBytes)),
		decodedBytes: optionalMedian(samples.map((sample) => sample.decodedBytes)),
		serverTiming: Object.fromEntries(
			metricNames.map((name) => [
				name,
				roundedMedian(samples.map((sample) => sample.serverTiming[name]))
			])
		)
	};
}
