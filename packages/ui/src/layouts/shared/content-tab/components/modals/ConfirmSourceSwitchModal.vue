<template>
	<NewModal ref="modal" :header="formatMessage(messages.header)" max-width="500px">
		<p class="m-0 text-primary">
			{{ formatMessage(messages.body, { project: projectName, source: targetSource }) }}
		</p>

		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" @click="confirm">
					<ArrowLeftRightIcon />
					{{ formatMessage(commonMessages.confirmButton) }}
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
	body: {
		id: 'instance.confirm-source-switch.body',
		defaultMessage:
			'{project} will be re-downloaded from {source}. Only continue if this exact version is available there — its updates will then come from {source} instead.',
	},
})

defineProps<{
	projectName: string
	targetSource: string
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
