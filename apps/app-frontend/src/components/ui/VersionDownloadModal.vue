<script setup lang="ts">
import { DownloadIcon } from '@modrinth/assets'
import { Button, DropdownSelect } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'

import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'

interface DownloadableVersion {
	id: string
	name?: string
	version_number: string
	game_versions: string[]
	loaders: string[]
	date_published?: string
	files: { url: string; primary?: boolean }[]
}

const props = defineProps<{
	title?: string
}>()

const modal = ref<InstanceType<typeof ModalWrapper> | null>(null)
const versions = ref<DownloadableVersion[]>([])
const selectedGameVersion = ref<string | null>(null)
const selectedLoader = ref<string | null>(null)

function show(payload: { title?: string; versions: DownloadableVersion[] }) {
	versions.value = [...payload.versions].sort(
		(a, b) => new Date(b.date_published ?? 0).getTime() - new Date(a.date_published ?? 0).getTime(),
	)
	selectedGameVersion.value = versions.value[0]?.game_versions?.[0] ?? null
	selectedLoader.value = versions.value[0]?.loaders?.[0] ?? null
	modal.value?.show()
}

defineExpose({ show })

const gameVersions = computed(() => [...new Set(versions.value.flatMap((v) => v.game_versions))])

const loaders = computed(() => [...new Set(versions.value.flatMap((v) => v.loaders))])

const filteredVersions = computed(() =>
	versions.value.filter(
		(v) =>
			(!selectedGameVersion.value || v.game_versions.includes(selectedGameVersion.value)) &&
			(!selectedLoader.value || v.loaders.includes(selectedLoader.value)),
	),
)

function download(version: DownloadableVersion) {
	const file = version.files.find((f) => f.primary) ?? version.files[0]
	if (file) void openUrl(file.url)
}
</script>

<template>
	<ModalWrapper ref="modal" header="Download version" :on-hide="() => {}">
		<div class="flex flex-col gap-4">
			<p v-if="props.title" class="m-0 text-secondary">{{ props.title }}</p>
			<div class="flex flex-wrap gap-3">
				<DropdownSelect
					v-if="gameVersions.length > 0"
					v-model="selectedGameVersion"
					name="download-game-version"
					:options="gameVersions"
					placeholder="Game version"
					class="flex-1"
				/>
				<DropdownSelect
					v-if="loaders.length > 1"
					v-model="selectedLoader"
					name="download-loader"
					:options="loaders"
					placeholder="Loader"
					class="flex-1"
				/>
			</div>
			<div class="flex max-h-96 flex-col gap-2 overflow-y-auto">
				<div
					v-for="version in filteredVersions"
					:key="version.id"
					class="flex items-center justify-between gap-4 rounded-xl bg-surface-3 p-3"
				>
					<div class="flex min-w-0 flex-col">
						<span class="truncate font-semibold text-contrast">{{ version.version_number }}</span>
						<span class="truncate text-sm text-secondary">
							{{ version.loaders.join(', ') }} &middot; {{ version.game_versions.join(', ') }}
						</span>
					</div>
					<Button type="outlined" class="shrink-0" @click="download(version)">
						<DownloadIcon />
						Download
					</Button>
				</div>
				<p v-if="filteredVersions.length === 0" class="m-0 text-secondary">
					No versions match your selection.
				</p>
			</div>
		</div>
	</ModalWrapper>
</template>
