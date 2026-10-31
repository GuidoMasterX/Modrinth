import { createConsoleState } from '@modrinth/ui'

import { clear_log_buffer, get_live_log_chunk, get_logs } from '@/helpers/logs'

type ConsoleState = ReturnType<typeof createConsoleState>

interface LogEntry {
	filename: string
	name?: string
	log_type: string
	output?: string | null
	age?: number
	live?: boolean
}

interface InstanceConsoleEntry {
	liveConsole: ConsoleState
	historicalConsole: ConsoleState
	historicalCache: Map<string, string>
	logList: LogEntry[] | null
	liveCursor: number
	liveGeneration: number
	epoch: number
	syncing: boolean
}

const instances = new Map<string, InstanceConsoleEntry>()

function getOrCreate(instanceId: string): InstanceConsoleEntry {
	let entry = instances.get(instanceId)
	if (entry) return entry

	entry = {
		liveConsole: createConsoleState(),
		historicalConsole: createConsoleState(),
		historicalCache: new Map(),
		logList: null,
		liveCursor: 0,
		liveGeneration: 0,
		epoch: 0,
		syncing: false,
	}
	instances.set(instanceId, entry)
	return entry
}

async function hydrate(instanceId: string): Promise<void> {
	await syncLiveBuffer(instanceId)
}

// Polls the backend's live log ring buffer and appends only lines written since
// the last cursor. App events can be dropped under heavy log volume, so the
// live view reconciles against the buffer instead of trusting them.
async function syncLiveBuffer(instanceId: string): Promise<void> {
	const entry = getOrCreate(instanceId)
	if (entry.syncing) return
	entry.syncing = true

	try {
		const epoch = entry.epoch
		const chunk = await get_live_log_chunk(instanceId, entry.liveCursor, entry.liveGeneration)
		if (entry.epoch !== epoch) return
		if (
			chunk.reset ||
			chunk.cursor < entry.liveCursor ||
			chunk.generation !== entry.liveGeneration
		) {
			// The buffer wrapped, was cleared, or restarted; rebuild.
			entry.liveConsole.clear()
		}
		if (chunk.output) entry.liveConsole.addLegacyLog(chunk.output)
		entry.liveCursor = chunk.cursor
		entry.liveGeneration = chunk.generation
	} finally {
		entry.syncing = false
	}
}

async function getHistoricalLogs(instanceId: string): Promise<LogEntry[]> {
	const entry = getOrCreate(instanceId)
	if (entry.logList) return entry.logList

	const logs: LogEntry[] = await get_logs(instanceId, true)
	entry.logList = logs

	for (const log of logs) {
		if (log.output) {
			entry.historicalCache.set(log.filename, log.output)
		}
	}

	return logs
}

function getHistoricalContent(instanceId: string, filename: string): string | undefined {
	return instances.get(instanceId)?.historicalCache.get(filename)
}

function invalidate(instanceId: string): void {
	const entry = instances.get(instanceId)
	if (!entry) return
	entry.historicalCache.clear()
	entry.logList = null
}

async function clearLive(instanceId: string): Promise<void> {
	const entry = getOrCreate(instanceId)
	entry.epoch += 1
	entry.liveConsole.clear()
	entry.liveCursor = 0
	entry.liveGeneration = 0
	await clear_log_buffer(instanceId).catch(() => {})
}

function resetLive(instanceId: string): void {
	const entry = getOrCreate(instanceId)
	entry.epoch += 1
	entry.liveConsole.clear()
	entry.liveCursor = 0
	entry.liveGeneration = 0
}

async function destroy(instanceId: string): Promise<void> {
	instances.delete(instanceId)
	await clear_log_buffer(instanceId).catch(() => {})
}

export function useInstanceConsole(instanceId: string) {
	const entry = getOrCreate(instanceId)
	return {
		liveConsole: entry.liveConsole,
		historicalConsole: entry.historicalConsole,
		hydrate: () => hydrate(instanceId),
		syncLiveBuffer: () => syncLiveBuffer(instanceId),
		getHistoricalLogs: () => getHistoricalLogs(instanceId),
		getHistoricalContent: (filename: string) => getHistoricalContent(instanceId, filename),
		invalidate: () => invalidate(instanceId),
		clearLive: () => clearLive(instanceId),
		resetLive: () => resetLive(instanceId),
		destroy: () => destroy(instanceId),
	}
}
