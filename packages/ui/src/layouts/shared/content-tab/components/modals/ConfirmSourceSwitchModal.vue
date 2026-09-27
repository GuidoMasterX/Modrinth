<template>
	<NewModal ref="modal" :header="formatMessage(messages.header)" max-width="500px">
		<p class="m-0 text-primary">
			{{
				loading
					? formatMessage(messages.loadingBody, { source: targetSource })
					: available
						? formatMessage(messages.availableBody, {
								project: projectName,
								source: targetSource,
							})
						: formatMessage(messages.unavailableBody, {
								project: projectName,
								source: targetSource,
							})
			}}
		</p>

		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button v-if="available" type="colored" color="brand" :disabled="loading" @click="confirm">
					<ArrowLeftRightIcon />
					{{ formatMessage(messages.switchButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import { ArrowLeftRightIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import { Button } from '#ui/components/base/buttons'
import NewModal from '#ui/components/modal/NewModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: {
		id: 'instance.confirm-source-switch.header',
		defaultMessage: 'Switch source?',
	},
	availableBody: {
		id: 'instance.confirm-source-switch.available-body',
		defaultMessage:
			'{project} is installed from one source and this exact version is also available on {source}. Switching re-downloads it from {source}, and future updates will come from there.',
	},
	unavailableBody: {
		id: 'instance.confirm-source-switch.unavailable-body',
		defaultMessage:
			'The installed version of {project} is not available on {source}, so it cannot be switched. {source} has to offer the exact same file.',
	},
	switchButton: {
		id: 'instance.confirm-source-switch.switch-button',
		defaultMessage: 'Switch',
	},
	loadingBody: {
		id: 'instance.confirm-source-switch.loading-body',
		defaultMessage: 'Checking whether this exact version is available on {source}…',
	},
})

defineProps<{
	projectName: string
	targetSource: string
	available: boolean
	loading?: boolean
}>()

const emit = defineEmits<{
	(e: 'switch'): void
}>()

const modal = ref<InstanceType<typeof NewModal>>()

function show() {
	modal.value?.show()
}

function confirm() {
	modal.value?.hide()
	emit('switch')
}

defineExpose({
	show,
})
</script>
