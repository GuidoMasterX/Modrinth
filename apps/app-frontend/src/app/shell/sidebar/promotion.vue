<script setup lang="ts">
import { ArrowBigUpDashIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { useResizeObserver } from '@vueuse/core'
import { ref } from 'vue'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	upgradeToModrinthPlus: {
		id: 'app.nav.upgrade-to-modrinth-plus',
		defaultMessage: 'Upgrade to Modrinth+',
	},
})
const upgradeLink = ref<HTMLElement | null>(null)
const emit = defineEmits<{
	'upgrade-height': [height: number]
}>()

useResizeObserver(upgradeLink, () => {
	emit('upgrade-height', upgradeLink.value?.getBoundingClientRect().height ?? 0)
})
</script>

<template>
	<a
		ref="upgradeLink"
		href="https://modrinth.plus?app"
		class="absolute bottom-4 w-full flex justify-center items-center gap-1 px-4 py-3 text-purple font-medium hover:underline z-10"
		target="_blank"
	>
		<ArrowBigUpDashIcon class="text-2xl" />
		{{ formatMessage(messages.upgradeToModrinthPlus) }}
	</a>
</template>
