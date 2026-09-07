<template>
	<Card>
		<ProjectPageDescription :description="project.body" />
		<ButtonLink :href="cfProjectUrl(project)" target="_blank" class="mt-4">
			<ExternalIcon />
			<span>{{ formatMessage(messages.readOnCurseForge) }}</span>
		</ButtonLink>
	</Card>
</template>

<script setup>
import { ExternalIcon } from '@modrinth/assets'
import { ButtonLink, Card, defineMessages, ProjectPageDescription, useVIntl } from '@modrinth/ui'

import { cfProjectUrl } from '@/helpers/curseforge-project'

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
