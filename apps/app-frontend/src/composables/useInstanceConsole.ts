import { createConsoleState } from '@modrinth/ui'

import { clear_log_buffer, get_live_log_buffer, get_logs } from '@/helpers/logs'

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
	liveLineCount: number
	liveTailLine: string | null
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
		liveLineCount: 0,
		liveTailLine: null,
		syncing: false,
	}
	instances.set(instanceId, entry)
	return entry
}

function splitBuffer(buffer: string): string[] {
	return buffer.split(/\r?\n/).filter((line, index, lines) => {
		return !(index === lines.length - 1 && line === '')
	})
}

async function hydrate(instanceId: string): Promise<void> {
	const entry = getOrCreate(instanceId)
	if (entry.liveConsole.output.value.length > 0) return

	const buffer = await get_live_log_buffer(instanceId)
	if (buffer) {
		entry.liveConsole.addLegacyLog(buffer)
		const lines = splitBuffer(buffer)
		entry.liveLineCount = lines.length
		entry.liveTailLine = lines[lines.length - 1] ?? null
	}
}

// Polls the backend's live log ring buffer and appends only newly emitted
// lines. App events can be dropped under heavy log volume, so the live view
// reconciles against the buffer instead of trusting them.
async function syncLiveBuffer(instanceId: string): Promise<void> {
	const entry = getOrCreate(instanceId)
	if (entry.syncing) return
	entry.syncing = true

	try {
		const buffer = await get_live_log_buffer(instanceId)
		if (!buffer) return

		const lines = splitBuffer(buffer)
		const tailLine = lines[lines.length - 1] ?? null

		if (lines.length < entry.liveLineCount) {
			entry.liveConsole.clear()
			entry.liveConsole.addLegacyLog(buffer)
			entry.liveLineCount = lines.length
			entry.liveTailLine = tailLine
			return
		}

		if (lines.length === entry.liveLineCount) {
			// ponytail: the ring buffer wraps only past 250k lines; assume a
			// single new line rather than rebuilding the whole view.
			if (tailLine !== null && tailLine !== entry.liveTailLine) {
				entry.liveConsole.addLegacyLog(tailLine)
				entry.liveTailLine = tailLine
			}
			return
		}

		const newLines = lines.slice(entry.liveLineCount)
		entry.liveConsole.addLegacyLog(newLines.join('\n'))
		entry.liveLineCount = lines.length
		entry.liveTailLine = tailLine
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
	entry.liveConsole.clear()
	entry.liveLineCount = 0
	entry.liveTailLine = null
	await clear_log_buffer(instanceId).catch(() => {})
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
		destroy: () => destroy(instanceId),
	}
}
