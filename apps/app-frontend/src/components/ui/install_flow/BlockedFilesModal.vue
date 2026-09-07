<template>
	<NewModal ref="modal" :header="formatMessage(messages.header)" fade="warning" max-width="550px">
		<p class="m-0 text-secondary">
			{{ formatMessage(messages.body) }}
		</p>
		<div class="mt-3 flex flex-col gap-2">
			<div
				v-for="file in blockedFiles"
				:key="`${file.project_id}-${file.file_name}`"
				class="bg-bg-raised border-rounded flex flex-col gap-1 p-3"
			>
				<div class="flex items-center justify-between gap-2">
					<span class="text-contrast font-medium">{{ file.project_name }}</span>
					<AutoLink :to="file.url" target="_blank" class="flex items-center gap-1 text-secondary">
						<ExternalIcon />
						<span>{{ formatMessage(messages.openOnCurseforge) }}</span>
					</AutoLink>
				</div>
				<span class="text-sm text-secondary">{{ file.file_name }}</span>
			</div>
		</div>

		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button @click="modal?.hide()">
					{{ formatMessage(commonMessages.closeButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { AutoLink, Button, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: {
		id: 'app.install.blocked-files.header',
		defaultMessage: 'Downloads blocked by CurseForge',
	},
	body: {
		id: 'app.install.blocked-files.body',
		defaultMessage:
			'The authors of the following files blocked third-party downloads, so they were not installed automatically. Download them manually from their CurseForge pages and add them to the instance — they will be recognized automatically.',
	},
	openOnCurseforge: {
		id: 'app.install.blocked-files.open-on-curseforge',
		defaultMessage: 'Open on CurseForge',
	},
})

const modal = ref<InstanceType<typeof NewModal>>()
const blockedFiles = ref<
	{ project_id: number; project_name: string; file_name: string; url: string }[]
>([])

function show(
	files: { project_id: number; project_name: string; file_name: string; url: string }[],
) {
	blockedFiles.value = files ?? []
	modal.value?.show()
}

defineExpose({
	show,
})
</script>
