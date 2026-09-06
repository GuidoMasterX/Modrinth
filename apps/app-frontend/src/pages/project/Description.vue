<template>
	<Card>
		<ProjectPageDescription :description="project.body" />
		<ButtonLink v-if="project.body_url" :href="project.body_url" target="_blank" class="mt-4">
			<ExternalIcon />
			<span>{{ formatMessage(messages.readOnCurseForge) }}</span>
		</ButtonLink>
	</Card>
</template>

<script setup>
import { ExternalIcon } from '@modrinth/assets'
import { ButtonLink, Card, defineMessages, ProjectPageDescription, useVIntl } from '@modrinth/ui'

defineProps({
	project: {
		type: Object,
		default: () => {},
	},
})

const { formatMessage } = useVIntl()

const messages = defineMessages({
	readOnCurseForge: {
		id: 'app.project.description.read-on-curseforge',
		defaultMessage: 'Read more on CurseForge',
	},
})
</script>

<script>
export default {
	name: 'Description',
}
</script>
