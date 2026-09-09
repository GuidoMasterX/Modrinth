<template>
	<div class="flex flex-col gap-2">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<span class="font-semibold text-contrast">{{
				formatMessage(messages.launcherInstancesTitle)
			}}</span>
			<Button
				type="quiet"
				size="xs"
				class="!h-6"
				:class="{ invisible: totalSelectedCount === 0 }"
				@click="clearAll"
				>{{ formatMessage(messages.clearAll) }}</Button
			>
		</div>

		<template v-if="loading">
			<div class="flex items-center justify-center py-8 text-secondary text-sm">
				{{ formatMessage(messages.detectingLauncherInstances) }}
			</div>
		</template>
		<template v-else>
			<!-- Search -->
			<Input
				v-if="ctx.importLaunchers.value.length > 0"
				v-model="ctx.importSearchQuery.value"
				:icon="SearchIcon"
				:placeholder="formatMessage(messages.searchInstanceNamePlaceholder)"
			/>

			<!-- Launcher sections -->
			<div v-if="ctx.importLaunchers.value.length > 0" class="flex flex-col gap-2">
				<div
					v-for="launcher in visibleLaunchers"
					:key="launcher.name"
					class="flex flex-col rounded-[20px] border border-solid border-surface-4 shadow-sm overflow-clip"
				>
					<!-- Launcher header -->
					<button
						class="flex w-full cursor-pointer items-center gap-3 border-none bg-surface-3 p-3 text-left transition-colors"
						@click="toggleLauncherExpanded(launcher.name)"
					>
						<ChevronRightIcon
							class="size-5 shrink-0 text-secondary transition-transform"
							:class="{ 'rotate-90': expandedLaunchers.has(launcher.name) }"
						/>
						<Checkbox
							:model-value="getLauncherCheckState(launcher)"
							:indeterminate="getLauncherIndeterminate(launcher)"
							@update:model-value="toggleLauncherAll(launcher, $event)"
							@click.stop
						/>
						<span class="font-semibold text-contrast">{{ launcher.name }}</span>
					</button>

					<!-- Instance list (expanded) -->
					<Collapsible :collapsed="!expandedLaunchers.has(launcher.name)">
						<div class="flex flex-col">
							<template v-for="(instance, i) in filteredInstances(launcher)" :key="instance">
								<div
									class="border-0 border-t border-solid border-surface-4 py-3 pr-3"
									:class="i % 2 === 0 ? 'bg-surface-2' : 'bg-surface-1.5'"
									style="padding-left: 2.75rem"
								>
									<div class="flex items-center gap-3">
										<Checkbox
											:model-value="isInstanceSelected(launcher.name, instance)"
											@update:model-value="toggleInstance(launcher.name, instance, $event)"
										/>
										<span class="text-sm">{{ instance }}</span>
									</div>
									<div
										v-if="
											isInstanceSelected(launcher.name, instance) &&
											instanceContents(launcher.name, instance)?.length
										"
										class="mt-2 ml-6 flex flex-col gap-1.5 rounded-xl border border-solid border-surface-4 bg-surface-3 p-2"
									>
										<span class="text-xs font-semibold text-secondary">
											{{ formatMessage(messages.chooseContentTitle) }}
										</span>
										<div
											v-for="entry in instanceContents(launcher.name, instance)"
											:key="entry.name"
											class="flex items-center gap-2"
										>
											<Checkbox
												:model-value="isContentSelected(launcher.name, instance, entry.name)"
												:disabled="isMandatoryContent(entry.name)"
												@update:model-value="
													toggleContent(launcher.name, instance, entry.name, $event)
												"
											/>
											<span class="text-sm">{{ entry.name }}</span>
											<span v-if="isMandatoryContent(entry.name)" class="text-xs text-secondary">
												({{ formatMessage(messages.mandatoryContent) }})
											</span>
										</div>
									</div>
								</div>
							</template>
						</div>
					</Collapsible>
				</div>
			</div>

			<!-- Add launcher path -->
			<div v-if="!showAddPath">
				<Button class="w-full" @click="showAddPath = true">
					{{ formatMessage(messages.addLauncherPath) }}
				</Button>
			</div>
			<div v-else class="flex items-center gap-2">
				<IconButton label="Browse for launcher path" @click="browseForLauncherPath">
					<FolderSearchIcon />
				</IconButton>
				<Input
					v-model="newLauncherPath"
					:placeholder="formatMessage(messages.launcherPathPlaceholder)"
					class="flex-1"
				/>
				<Button :disabled="!newLauncherPath.trim()" @click="addLauncherPath">
					{{ formatMessage(messages.add) }}
				</Button>
			</div>
		</template>
	</div>
</template>

<script setup lang="ts">
import { ChevronRightIcon, FolderSearchIcon, SearchIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'

import { Button, IconButton } from '#ui/components/base/buttons'

import { injectInstanceImport, injectNotificationManager } from '../../../../providers'
import type {
	ImportableLauncher,
	ImportInstanceContentEntry,
} from '../../../../providers/instance-import'
import Checkbox from '../../../base/Checkbox.vue'
import Collapsible from '../../../base/Collapsible.vue'
import Input from '../../../base/inputs/Input.vue'
import { injectCreationFlowContext } from '../creation-flow-context'

const ctx = injectCreationFlowContext()
const importProvider = injectInstanceImport()
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()

const loading = ref(false)
const expandedLaunchers = ref(new Set<string>())
const expandedBeforeSearch = ref<Set<string> | null>(null)
const showAddPath = ref(false)
const newLauncherPath = ref('')

const messages = defineMessages({
	launcherInstancesTitle: {
		id: 'creation-flow.modal.import-instance.launcher-instances.title',
		defaultMessage: 'Launcher instances',
	},
	clearAll: {
		id: 'creation-flow.modal.import-instance.selection.clear-all',
		defaultMessage: 'Clear all',
	},
	detectingLauncherInstances: {
		id: 'creation-flow.modal.import-instance.detecting-launcher-instances',
		defaultMessage: 'Detecting launcher instances...',
	},
	searchInstanceNamePlaceholder: {
		id: 'creation-flow.modal.import-instance.search.placeholder',
		defaultMessage: 'Search instance name',
	},
	addLauncherPath: {
		id: 'creation-flow.modal.import-instance.launcher-path.add',
		defaultMessage: 'Add launcher path',
	},
	launcherPathPlaceholder: {
		id: 'creation-flow.modal.import-instance.launcher-path.placeholder',
		defaultMessage: 'Path to launcher...',
	},
	add: {
		id: 'creation-flow.modal.import-instance.action.add',
		defaultMessage: 'Add',
	},
	noInstancesFoundTitle: {
		id: 'creation-flow.modal.import-instance.notification.no-instances-found.title',
		defaultMessage: 'No instances found',
	},
	noInstancesFoundText: {
		id: 'creation-flow.modal.import-instance.notification.no-instances-found.text',
		defaultMessage: 'No importable instances were found at the specified path.',
	},
	customLauncherName: {
		id: 'creation-flow.modal.import-instance.custom-launcher.name',
		defaultMessage: 'Custom ({pathName})',
	},
	chooseContentTitle: {
		id: 'creation-flow.modal.import-instance.content.title',
		defaultMessage: 'Choose what to import',
	},
	mandatoryContent: {
		id: 'creation-flow.modal.import-instance.content.mandatory',
		defaultMessage: 'Required',
	},
})

// Load detected launchers on mount
onMounted(async () => {
	if (ctx.importLaunchers.value.length > 0) return // Already loaded

	loading.value = true
	try {
		ctx.importLaunchers.value = await importProvider.getDetectedLaunchers()
		// Auto-expand launchers that have instances
		for (const launcher of ctx.importLaunchers.value) {
			if (launcher.instances.length > 0) {
				expandedLaunchers.value.add(launcher.name)
			}
		}
	} catch {
		ctx.importLaunchers.value = []
	}
	loading.value = false
})

// Filter instances by search query
function filteredInstances(launcher: ImportableLauncher): string[] {
	const query = ctx.importSearchQuery.value.toLowerCase().trim()
	if (!query) return launcher.instances
	return launcher.instances.filter((name) => name.toLowerCase().includes(query))
}

// Hide launchers with no matching instances when searching
const visibleLaunchers = computed(() => {
	const query = ctx.importSearchQuery.value.toLowerCase().trim()
	if (!query) return ctx.importLaunchers.value
	return ctx.importLaunchers.value.filter((launcher) => filteredInstances(launcher).length > 0)
})

// Auto-expand launchers with matching results when searching
watch(
	() => ctx.importSearchQuery.value,
	(query) => {
		const trimmed = query.trim()
		if (trimmed) {
			// Save current state before search overrides it
			if (!expandedBeforeSearch.value) {
				expandedBeforeSearch.value = new Set(expandedLaunchers.value)
			}
			// Expand all launchers that have matching instances
			const newExpanded = new Set(expandedLaunchers.value)
			for (const launcher of ctx.importLaunchers.value) {
				if (filteredInstances(launcher).length > 0) {
					newExpanded.add(launcher.name)
				}
			}
			expandedLaunchers.value = newExpanded
		} else if (expandedBeforeSearch.value) {
			// Restore pre-search state
			expandedLaunchers.value = expandedBeforeSearch.value
			expandedBeforeSearch.value = null
		}
	},
)

// Selection helpers
function isInstanceSelected(launcherName: string, instance: string): boolean {
	return ctx.importSelectedInstances.value[launcherName]?.has(instance) ?? false
}

const MANDATORY_CONTENT = 'minecraftinstance.json'

const contentsMap = ref<Record<string, ImportInstanceContentEntry[] | null>>({})
const contentsPending = new Set<string>()

function contentKey(launcherName: string, instance: string) {
	return `${launcherName}/${instance}`
}

function instanceContents(launcherName: string, instance: string) {
	return contentsMap.value[contentKey(launcherName, instance)] ?? null
}

function isMandatoryContent(name: string) {
	return name.toLowerCase() === MANDATORY_CONTENT
}

function setDefaultContentsSelection(key: string, entries: ImportInstanceContentEntry[]) {
	ctx.importSelectedContents.value[key] = new Set(entries.map((entry) => entry.name))
	ctx.importSelectedContents.value = { ...ctx.importSelectedContents.value }
}

function ensureContents(launcher: ImportableLauncher, instance: string) {
	const key = contentKey(launcher.name, instance)
	const cached = contentsMap.value[key]
	if (cached) {
		if (!ctx.importSelectedContents.value[key]) {
			setDefaultContentsSelection(key, cached)
		}
		return
	}
	if (contentsPending.has(key)) return
	contentsPending.add(key)
	importProvider
		.getImportableInstanceContents(launcher.name, launcher.path, instance)
		.then((entries) => {
			contentsMap.value[key] = entries
			if (entries) {
				setDefaultContentsSelection(key, entries)
			}
		})
		.catch(() => {})
		.finally(() => contentsPending.delete(key))
}

function isContentSelected(launcherName: string, instance: string, name: string): boolean {
	return ctx.importSelectedContents.value[contentKey(launcherName, instance)]?.has(name) ?? true
}

function toggleContent(launcherName: string, instance: string, name: string, selected: boolean) {
	const key = contentKey(launcherName, instance)
	const set = ctx.importSelectedContents.value[key] ?? new Set<string>()
	if (selected) {
		set.add(name)
	} else {
		set.delete(name)
	}
	ctx.importSelectedContents.value[key] = set
	ctx.importSelectedContents.value = { ...ctx.importSelectedContents.value }
}

function toggleInstance(launcherName: string, instance: string, selected: boolean) {
	if (!ctx.importSelectedInstances.value[launcherName]) {
		ctx.importSelectedInstances.value[launcherName] = new Set()
	}
	if (selected) {
		ctx.importSelectedInstances.value[launcherName].add(instance)
		const launcher = ctx.importLaunchers.value.find((l) => l.name === launcherName)
		if (launcher) void ensureContents(launcher, instance)
	} else {
		ctx.importSelectedInstances.value[launcherName].delete(instance)
		const key = contentKey(launcherName, instance)
		const { [key]: _removed, ...rest } = ctx.importSelectedContents.value
		ctx.importSelectedContents.value = rest
	}
	// Trigger reactivity
	ctx.importSelectedInstances.value = { ...ctx.importSelectedInstances.value }
}

function getLauncherCheckState(launcher: ImportableLauncher): boolean {
	const set = ctx.importSelectedInstances.value[launcher.name]
	if (!set || set.size === 0) return false
	const visible = filteredInstances(launcher)
	return visible.length > 0 && visible.every((i) => set.has(i))
}

function getLauncherIndeterminate(launcher: ImportableLauncher): boolean {
	const set = ctx.importSelectedInstances.value[launcher.name]
	if (!set || set.size === 0) return false
	const visible = filteredInstances(launcher)
	const selectedVisible = visible.filter((i) => set.has(i))
	return selectedVisible.length > 0 && selectedVisible.length < visible.length
}

function toggleLauncherAll(launcher: ImportableLauncher, selected: boolean) {
	if (!ctx.importSelectedInstances.value[launcher.name]) {
		ctx.importSelectedInstances.value[launcher.name] = new Set()
	}
	const visible = filteredInstances(launcher)
	let removedContents = false
	for (const instance of visible) {
		if (selected) {
			ctx.importSelectedInstances.value[launcher.name].add(instance)
			void ensureContents(launcher, instance)
		} else {
			ctx.importSelectedInstances.value[launcher.name].delete(instance)
			const key = contentKey(launcher.name, instance)
			if (ctx.importSelectedContents.value[key] !== undefined) {
				const { [key]: _removed, ...rest } = ctx.importSelectedContents.value
				ctx.importSelectedContents.value = rest
				removedContents = true
			}
		}
	}
	if (!selected && removedContents) {
		ctx.importSelectedContents.value = {
			...ctx.importSelectedContents.value,
		}
	}
	// Trigger reactivity
	ctx.importSelectedInstances.value = { ...ctx.importSelectedInstances.value }
}

function toggleLauncherExpanded(name: string) {
	if (expandedLaunchers.value.has(name)) {
		expandedLaunchers.value.delete(name)
	} else {
		expandedLaunchers.value.add(name)
	}
	expandedLaunchers.value = new Set(expandedLaunchers.value)
}

const totalSelectedCount = computed(() => {
	let count = 0
	for (const set of Object.values(ctx.importSelectedInstances.value)) {
		count += set.size
	}
	return count
})

function clearAll() {
	ctx.importSelectedInstances.value = {}
	ctx.importSelectedContents.value = {}
}

async function browseForLauncherPath() {
	const path = await importProvider.selectDirectory()
	if (path) {
		newLauncherPath.value = path
	}
}

async function addLauncherPath() {
	const path = newLauncherPath.value.trim()
	if (!path) return

	try {
		const instances = await importProvider.getImportableInstances('Custom', path)
		if (instances.length === 0) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.noInstancesFoundTitle),
				text: formatMessage(messages.noInstancesFoundText),
			})
			return
		}
		const launcher: ImportableLauncher = {
			name: formatMessage(messages.customLauncherName, {
				pathName: path.split(/[\\/]/).pop() || path,
			}),
			path,
			instances,
		}
		ctx.importLaunchers.value = [...ctx.importLaunchers.value, launcher]
		expandedLaunchers.value.add(launcher.name)
		expandedLaunchers.value = new Set(expandedLaunchers.value)
	} catch {
		addNotification({
			type: 'error',
			title: formatMessage(messages.noInstancesFoundTitle),
			text: formatMessage(messages.noInstancesFoundText),
		})
		return
	}

	newLauncherPath.value = ''
	showAddPath.value = false
}
</script>
