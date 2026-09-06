<script setup lang="ts">
import { KeyIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, Input, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { ref, watch } from 'vue'

import { useCurseforgeKey } from '@/composables/use-curseforge-key.ts'
import { get, set } from '@/helpers/settings.ts'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const settings = ref(await get())
const { setCurseforgeKey } = useCurseforgeKey()

const messages = defineMessages({
	title: {
		id: 'app.settings.integrations.title',
		defaultMessage: 'Integrations',
	},
	curseforgeTitle: {
		id: 'app.settings.integrations.curseforge.title',
		defaultMessage: 'CurseForge',
	},
	curseforgeDescription: {
		id: 'app.settings.integrations.curseforge.description',
		defaultMessage:
			'Enter your personal CurseForge API key to browse and install CurseForge mods, resource packs, shader packs, and modpacks. You can generate a key from the CurseForge developer console.',
	},
	curseforgePlaceholder: {
		id: 'app.settings.integrations.curseforge.placeholder',
		defaultMessage: 'Paste your CurseForge API key',
	},
	consoleLink: {
		id: 'app.settings.integrations.curseforge.console-link',
		defaultMessage: 'Open the CurseForge developer console',
	},
})

async function save() {
	try {
		await set(settings.value)
		setCurseforgeKey(settings.value.curseforge_api_key)
	} catch (error) {
		handleError(error as Error)
	}
}

function openConsole() {
	void openUrl('https://console.curseforge.com/s/api-keys')
}

watch(settings, save, { deep: true })
</script>

<template>
	<h2 class="m-0 text-lg font-semibold text-contrast">
		{{ formatMessage(messages.curseforgeTitle) }}
	</h2>
	<p class="m-0 mt-1">
		{{ formatMessage(messages.curseforgeDescription) }}
	</p>
	<div class="mt-4 flex max-w-lg items-center gap-2">
		<Input
			v-model="settings.curseforge_api_key"
			:placeholder="formatMessage(messages.curseforgePlaceholder)"
			:icon="KeyIcon"
			:clearable="true"
			type="password"
			autocomplete="off"
		/>
	</div>
	<Button class="mt-4" @click="openConsole">
		{{ formatMessage(messages.consoleLink) }}
	</Button>
</template>
