<template>
	<div class="flex items-center gap-1">
		<Button
			v-if="showClear && hasLogs"
			v-tooltip="clearDisabled ? clearDisabledTooltip : undefined"
			type="quiet"
			class="!text-sm !font-medium"
			:disabled="clearDisabled"
			@click="emit('clear')"
		>
			<XIcon aria-hidden="true" />
			{{ formatMessage(commonMessages.clearButton) }}
		</Button>
		<Button
			v-if="showDelete"
			v-tooltip="deleteDisabled ? deleteDisabledTooltip : undefined"
			type="quiet"
			class="!text-sm !font-medium"
			color="red"
			interaction="filled"
			:disabled="deleteDisabled"
			@click="emit('delete')"
		>
			<TrashIcon aria-hidden="true" />
			{{ formatMessage(commonMessages.deleteLabel) }}
		</Button>
		<Button
			v-if="hasLogs"
			v-tooltip="shareDisabled ? shareDisabledTooltip : undefined"
			type="quiet"
			class="!text-sm !font-medium"
			:disabled="shareDisabled"
			:loading="sharing"
			@click="emit('share')"
		>
			<SpinnerIcon v-if="sharing" class="animate-spin" aria-hidden="true" />
			<ShareIcon v-else aria-hidden="true" />
			{{ formatMessage(messages.share) }}
		</Button>
		<Button type="quiet" class="!text-sm !font-medium" @click="emit('toggle-fullscreen')">
			<ContractIcon v-if="fullscreen" aria-hidden="true" />
			<ExpandIcon v-else aria-hidden="true" />
			{{ formatMessage(fullscreen ? messages.collapse : messages.expand) }}
		</Button>
		<FloatingMenu placement="top-end" :distance="6">
			<Button type="quiet" class="!text-sm !font-medium">
				<SettingsIcon aria-hidden="true" />
				{{ formatMessage(messages.options) }}
			</Button>
			<template #popper="{ hide }">
				<div class="flex w-52 flex-col p-1">
					<span class="px-3 py-1.5 text-xs font-semibold text-secondary">{{
						formatMessage(messages.linesShown)
					}}</span>
					<button
						v-for="option in RENDER_LIMIT_OPTIONS"
						:key="option.value"
						type="button"
						class="flex items-center gap-2 rounded-lg px-3 py-1.5 text-left text-sm font-medium text-contrast hover:bg-surface-4"
						@click="emit('update:renderLimit', option.value); hide()"
					>
						<CheckIcon v-if="renderLimit === option.value" class="size-4 shrink-0" />
						<span v-else class="size-4 shrink-0" />
						{{ option.label }}
					</button>
				</div>
			</template>
		</FloatingMenu>
	</div>
</template>

<script setup lang="ts">
import {
	CheckIcon,
	ContractIcon,
	ExpandIcon,
	SettingsIcon,
	ShareIcon,
	SpinnerIcon,
	TrashIcon,
	XIcon,
} from '@modrinth/assets'

import { Button } from '#ui/components/base/buttons'
import FloatingMenu from '#ui/components/floating/FloatingMenu.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const RENDER_LIMIT_OPTIONS = [
	{ value: 0, label: 'All lines' },
	{ value: 100_000, label: '100,000 lines' },
	{ value: 25_000, label: '25,000 lines' },
	{ value: 5_000, label: '5,000 lines' },
] as const

const { formatMessage } = useVIntl()
const messages = defineMessages({
	share: {
		id: 'console.actions.share',
		defaultMessage: 'Share',
	},
	collapse: {
		id: 'console.actions.collapse',
		defaultMessage: 'Collapse',
	},
	expand: {
		id: 'console.actions.expand',
		defaultMessage: 'Expand',
	},
	options: {
		id: 'console.actions.options',
		defaultMessage: 'Options',
	},
	linesShown: {
		id: 'console.actions.lines-shown',
		defaultMessage: 'Lines shown',
	},
})

defineProps<{
	showClear?: boolean
	hasLogs?: boolean
	shareDisabled?: boolean
	shareDisabledTooltip?: string
	sharing?: boolean
	fullscreen?: boolean
	clearDisabled?: boolean
	clearDisabledTooltip?: string
	showDelete?: boolean
	deleteDisabled?: boolean
	deleteDisabledTooltip?: string
	renderLimit?: number
}>()

const emit = defineEmits<{
	clear: []
	share: []
	'toggle-fullscreen': []
	delete: []
	'update:renderLimit': [value: number]
}>()
</script>
