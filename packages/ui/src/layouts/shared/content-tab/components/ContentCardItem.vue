<script setup lang="ts">
import {
	ArrowLeftRightIcon,
	DownloadIcon,
	LinkIcon,
	LockIcon,
	MoreVerticalIcon,
	SpinnerIcon,
	TrashExclamationIcon,
	TrashIcon,
	TriangleAlertIcon,
	UploadIcon,
} from '@modrinth/assets'
import { capitalizeString } from '@modrinth/utils'
import { computed, getCurrentInstance, ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import AutoLink from '#ui/components/base/AutoLink.vue'
import Avatar from '#ui/components/base/Avatar.vue'
import BulletDivider from '#ui/components/base/BulletDivider.vue'
import type { ButtonMenuOption } from '#ui/components/base/buttons'
import { IconButton, TeleportOverflowMenu } from '#ui/components/base/buttons'
import Checkbox from '#ui/components/base/Checkbox.vue'
import FormattedTag from '#ui/components/base/FormattedTag.vue'
import ProgressSpinner from '#ui/components/base/ProgressSpinner.vue'
import Toggle from '#ui/components/base/Toggle.vue'
import { useCompactNumber } from '#ui/composables'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'
import { useShiftKey } from '#ui/utils/shift-key'
import { truncatedTooltip } from '#ui/utils/truncate'

import type {
	ClientWarningType,
	ContentCardProject,
	ContentCardVersion,
	ContentOwner,
	ContentSource,
} from '../types'

const { formatMessage } = useVIntl()
const { formatCompactNumber } = useCompactNumber()

const messages = defineMessages({
	selectProject: {
		id: 'content.card.select-project',
		defaultMessage: 'Select {project}',
	},
	uploaded: {
		id: 'content.card.uploaded',
		defaultMessage: 'Uploaded',
	},
	frozen: {
		id: 'content.card.frozen',
		defaultMessage: 'This project is locked to its current version until unfrozen.',
	},
	curseforge: {
		id: 'content.card.source.curseforge',
		defaultMessage: 'CurseForge',
	},
	modrinth: {
		id: 'content.card.source.modrinth',
		defaultMessage: 'Modrinth',
	},
	external: {
		id: 'content.card.source.external',
		defaultMessage: 'External',
	},
	synced: {
		id: 'content.card.synced',
		defaultMessage: 'Synced across instances',
	},
	syncUpdatePending: {
		id: 'content.card.sync-update-pending',
		defaultMessage:
			'Some synced copies are waiting for changes. An instance may be running, have a frozen or incompatible version, or already contain its own copy.',
	},
})

interface Props {
	project: ContentCardProject
	projectLink?: string | RouteLocationRaw
	version?: ContentCardVersion
	showVersion?: boolean
	versionLink?: string | RouteLocationRaw
	owner?: ContentOwner
	source?: ContentSource
	packageSource?: 'modrinth' | 'curseforge' | null
	externalUrl?: string
	external?: boolean
	enabled?: boolean
	locked?: boolean
	installing?: boolean
	installProgress?: number | null
	hasUpdate?: boolean
	isClientOnly?: boolean
	clientWarning?: ClientWarningType | null
	synced?: boolean
	syncUpdatePending?: boolean
	hideSwitchVersion?: boolean
	overflowOptions?: ButtonMenuOption[]
	disabled?: boolean
	disabledTooltip?: string | null
	toggleDisabled?: boolean
	toggleDisabledTooltip?: string | null
	hideToggle?: boolean
	showCheckbox?: boolean
	hideDelete?: boolean
	hideActions?: boolean
	inline?: boolean
}

const props = withDefaults(defineProps<Props>(), {
	projectLink: undefined,
	version: undefined,
	showVersion: true,
	versionLink: undefined,
	owner: undefined,
	source: undefined,
	packageSource: undefined,
	externalUrl: undefined,
	external: false,
	enabled: undefined,
	locked: false,
	installing: false,
	installProgress: undefined,
	hasUpdate: false,
	isClientOnly: false,
	clientWarning: null,
	synced: false,
	syncUpdatePending: false,
	hideSwitchVersion: false,
	overflowOptions: undefined,
	disabled: false,
	disabledTooltip: undefined,
	toggleDisabled: false,
	toggleDisabledTooltip: undefined,
	hideToggle: false,
	showCheckbox: false,
	hideDelete: false,
	hideActions: false,
	inline: false,
})

const selected = defineModel<boolean>('selected')

const emit = defineEmits<{
	'update:enabled': [value: boolean]
	select: [value: boolean, event?: MouseEvent]
	delete: [event: MouseEvent]
	update: []
	switchVersion: []
}>()

const instance = getCurrentInstance()
const hasDeleteListener = computed(() => typeof instance?.vnode.props?.onDelete === 'function')
const hasUpdateListener = computed(() => typeof instance?.vnode.props?.onUpdate === 'function')
const hasSwitchVersionListener = computed(
	() => typeof instance?.vnode.props?.onSwitchVersion === 'function',
)

const versionNumberRef = ref<HTMLElement | null>(null)
const fileNameRef = ref<HTMLElement | null>(null)

const isDisabled = computed(() => props.disabled || props.installing)
const isToggleDisabled = computed(() => isDisabled.value || props.toggleDisabled)
const syncStatusLabel = computed(() =>
	formatMessage(props.syncUpdatePending ? messages.syncUpdatePending : messages.synced),
)

const clientWarningMessage = computed(() => {
	switch (props.clientWarning) {
		case 'retained':
			return commonMessages.clientRetainedWarning
		case 'depends':
			return commonMessages.clientDependsWarning
		default:
			return commonMessages.clientOnlyWarning
	}
})

const { shift: shiftHeld } = useShiftKey()
const deleteHovered = ref(false)
const installTooltip = computed(() => {
	if (!props.installing) return undefined
	if (props.installProgress == null) return formatMessage(commonMessages.installingLabel)
	return `${formatMessage(commonMessages.installingLabel)} (${Math.round(props.installProgress)}%)`
})

const sourcePillColor = computed(() =>
	props.packageSource === 'curseforge'
		? 'var(--color-source-curseforge)'
		: props.packageSource === 'modrinth'
			? 'var(--color-source-modrinth)'
			: 'var(--color-source-external)',
)
const sourcePillMessage = computed(() =>
	props.packageSource === 'curseforge'
		? messages.curseforge
		: props.packageSource === 'modrinth'
			? messages.modrinth
			: messages.external,
)
</script>

<template>
	<div
		role="row"
		class="flex items-center justify-between"
		:class="{
			'h-[84px] gap-4 px-3': !inline,
			'gap-3': inline,
			'opacity-50 grayscale': disabled && !installing,
			'opacity-50': installing,
		}"
	>
		<div
			class="flex min-w-0 items-center gap-4"
			:class="
				hideActions || !showVersion
					? 'flex-1'
					: 'flex-1 @[800px]:w-[45%] @[800px]:shrink-0 @[800px]:flex-none'
			"
		>
			<Checkbox
				v-if="showCheckbox"
				:model-value="selected ?? false"
				:aria-label="formatMessage(messages.selectProject, { project: project.title })"
				:disabled="isDisabled"
				class="shrink-0"
				@update:model-value="(value, event) => emit('select', value, event)"
			/>

			<div
				class="flex min-w-0 items-center gap-3 transition-[filter,opacity] duration-200"
				:class="enabled === false && !disabled ? 'grayscale opacity-50' : ''"
			>
				<div v-tooltip="installTooltip" class="relative flex shrink-0 items-center">
					<Avatar
						:src="project.icon_url"
						:alt="project.title"
						size="3rem"
						loading="lazy"
						no-shadow
						class="rounded-2xl border border-surface-5"
					/>
					<div
						v-if="installing"
						class="absolute inset-0 flex items-center justify-center rounded-2xl bg-black/20"
					>
						<ProgressSpinner
							v-if="installProgress != null && installProgress > 0"
							:progress="installProgress"
							:max="100"
							class="size-5 text-white"
						/>
						<SpinnerIcon v-else class="size-5 animate-spin text-white" />
					</div>
				</div>
				<div class="flex min-w-0 flex-col gap-0.5">
					<div class="flex min-w-0 items-center gap-1">
						<AutoLink
							:target="
								typeof projectLink === 'string' && projectLink.startsWith('http')
									? '_blank'
									: undefined
							"
							:to="projectLink"
							class="truncate text-base font-semibold leading-6 text-contrast !decoration-contrast"
							:class="{ 'hover:underline': projectLink }"
						>
							{{ project.title }}
						</AutoLink>
						<slot name="title-badges" />
						<AutoLink
							v-if="packageSource === 'curseforge'"
							v-tooltip="formatMessage(messages.curseforge)"
							:to="externalUrl"
							target="_blank"
							class="inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-1 text-sm font-semibold leading-none @[800px]:hidden"
							:style="{
								backgroundColor:
									'color-mix(in srgb, var(--color-source-curseforge) 18%, transparent)',
								color: 'var(--color-source-curseforge)',
							}"
						>
							<span
								class="size-2 rounded-full"
								:style="{ backgroundColor: 'var(--color-source-curseforge)' }"
							/>
							{{ formatMessage(messages.curseforge) }}
						</AutoLink>
						<span
							v-if="synced && hideActions"
							v-tooltip="syncStatusLabel"
							:aria-label="syncStatusLabel"
							role="img"
							class="inline-flex size-5 shrink-0 cursor-help items-center justify-center"
							tabindex="0"
						>
							<LinkIcon class="size-4 text-blue" aria-hidden="true" />
						</span>
						<span
							v-if="isClientOnly"
							v-tooltip="formatMessage(clientWarningMessage)"
							class="inline-flex size-5 shrink-0 cursor-help items-center justify-center"
							tabindex="0"
						>
							<TriangleAlertIcon class="pointer-events-none size-4 text-orange" />
						</span>
					</div>

					<div class="flex min-w-0 items-center gap-1">
						<template v-if="source">
							<AutoLink
								:target="
									typeof source.link === 'string' && source.link.startsWith('http')
										? '_blank'
										: undefined
								"
								:to="source.link"
								class="flex min-w-0 items-center gap-1 !decoration-secondary"
								:class="{ 'hover:underline': source.link }"
							>
								<Avatar
									:src="source.project.icon_url"
									:alt="source.project.title"
									:tint-by="source.project.id"
									size="1.25rem"
									loading="lazy"
									no-shadow
									class="shrink-0 rounded-md"
								/>
								<span class="truncate text-sm leading-5 text-secondary">
									{{ source.project.title }}
								</span>
							</AutoLink>
						</template>
						<AutoLink
							v-else-if="owner"
							:target="
								typeof owner.link === 'string' && owner.link.startsWith('http')
									? '_blank'
									: undefined
							"
							:to="owner.link"
							class="flex shrink-0 items-center gap-1 !decoration-secondary"
							:class="{ 'hover:underline': owner.link }"
						>
							<Avatar
								:src="owner.avatar_url"
								:alt="owner.name"
								size="1.5rem"
								:circle="owner.type === 'user'"
								loading="lazy"
								no-shadow
								class="shrink-0"
							/>
							<span class="text-sm leading-5 text-secondary">{{ owner.name }}</span>
						</AutoLink>
						<span v-else-if="external" class="flex items-center gap-1 text-secondary">
							<UploadIcon class="size-4 shrink-0" />
							<span class="text-sm leading-5">{{ formatMessage(messages.uploaded) }}</span>
						</span>
						<template v-if="showVersion && version && !external">
							<BulletDivider class="shrink-0 @[800px]:hidden" />
							<AutoLink
								:target="
									typeof versionLink === 'string' && versionLink.startsWith('http')
										? '_blank'
										: undefined
								"
								:to="versionLink"
								class="truncate text-sm leading-5 text-secondary !decoration-secondary @[800px]:hidden"
								:class="{ 'hover:underline': versionLink }"
							>
								{{ version.version_number }}
							</AutoLink>
						</template>
						<template v-if="project.downloads || project.categories?.length">
							<BulletDivider class="shrink-0" />
							<span
								v-if="project.downloads"
								v-tooltip="
									capitalizeString(
										formatMessage(commonMessages.projectDownloads, {
											count: project.downloads,
										}),
									)
								"
								class="flex shrink-0 items-center gap-1 text-sm leading-5 text-secondary"
							>
								<DownloadIcon class="size-4 shrink-0" aria-hidden="true" />
								{{ formatCompactNumber(project.downloads) }}
							</span>
							<template v-if="project.categories?.length">
								<BulletDivider v-if="project.downloads" class="shrink-0" />
								<span
									v-for="category in project.categories?.slice(0, 2)"
									:key="category"
									class="inline-flex shrink-0 items-center rounded-full border-[1px] border-solid border-surface-5 bg-[--_bg-color,var(--color-button-bg)] px-1.5 py-0.5 text-xs leading-none font-normal text-nowrap text-secondary"
								>
									<FormattedTag :tag="category" />
								</span>
							</template>
						</template>
					</div>
				</div>
			</div>
		</div>

		<div
			v-if="showVersion"
			class="hidden flex-col gap-0.5 transition-[filter,opacity] duration-200 @[800px]:flex"
			:class="[
				hideActions ? 'flex-1' : 'flex-1 min-w-0',
				enabled === false && !disabled ? 'grayscale opacity-50' : '',
			]"
		>
			<template v-if="version">
				<AutoLink
					v-tooltip="truncatedTooltip(versionNumberRef, version.version_number)"
					:target="
						typeof versionLink === 'string' && versionLink.startsWith('http') ? '_blank' : undefined
					"
					:to="versionLink"
					class="inline-flex min-w-0 font-semibold leading-6 text-contrast !decoration-contrast"
					:class="{ 'hover:underline': versionLink, 'cursor-pointer': versionLink }"
				>
					<span ref="versionNumberRef" class="truncate">{{
						version.version_number.slice(0, Math.ceil(version.version_number.length / 2))
					}}</span
					><span class="shrink-0">{{
						version.version_number.slice(Math.ceil(version.version_number.length / 2))
					}}</span>
				</AutoLink>
				<span
					v-tooltip="truncatedTooltip(fileNameRef, version.file_name)"
					class="flex min-w-0 leading-6 text-secondary"
				>
					<span ref="fileNameRef" class="truncate">{{
						version.file_name.slice(0, Math.ceil(version.file_name.length / 2))
					}}</span
					><span class="shrink-0">{{
						version.file_name.slice(Math.ceil(version.file_name.length / 2))
					}}</span>
				</span>
			</template>
		</div>

		<div class="hidden w-32 shrink-0 items-center @[800px]:flex">
			<span
				class="inline-flex w-28 shrink-0 items-center justify-center gap-1.5 rounded-full px-2 py-1 text-sm font-semibold leading-none"
				:style="{
					backgroundColor: `color-mix(in srgb, ${sourcePillColor} 18%, transparent)`,
					color: sourcePillColor,
				}"
			>
				<span class="size-2 rounded-full" :style="{ backgroundColor: sourcePillColor }" />
				{{ formatMessage(sourcePillMessage) }}
			</span>
		</div>

		<div
			v-if="!hideActions"
			class="flex min-w-[160px] shrink-0 items-center justify-end gap-2 transition-colors duration-200"
		>
			<slot name="additionalButtonsLeft" />
			<span
				v-if="synced"
				v-tooltip="syncStatusLabel"
				:aria-label="syncStatusLabel"
				role="img"
				tabindex="0"
				class="inline-flex size-9 shrink-0 cursor-help items-center justify-center rounded-xl text-blue focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
			>
				<LinkIcon class="size-5" aria-hidden="true" />
			</span>

			<!-- Fixed width container to reserve space for update/switch version button -->
			<div
				v-if="
					locked ||
					(hasUpdateListener && hasUpdate) ||
					(hasSwitchVersionListener && version && !hideSwitchVersion)
				"
				class="flex w-8 items-center justify-center"
			>
				<IconButton
					v-if="locked"
					v-tooltip="formatMessage(messages.frozen)"
					type="quiet"
					:label="formatMessage(messages.frozen)"
					disabled
				>
					<LockIcon class="size-5" />
				</IconButton>
				<IconButton
					v-else-if="hasUpdate"
					v-tooltip="
						isDisabled && disabledTooltip
							? disabledTooltip
							: formatMessage(commonMessages.updateAvailableLabel)
					"
					type="quiet"
					color="green"
					:label="
						isDisabled && disabledTooltip
							? disabledTooltip
							: formatMessage(commonMessages.updateAvailableLabel)
					"
					:disabled="isDisabled"
					class="hover:!bg-green focus-visible:!bg-green hover:!text-[var(--color-accent-contrast)] focus-visible:!text-[var(--color-accent-contrast)]"
					@click="emit('update')"
				>
					<DownloadIcon class="size-5" />
				</IconButton>
				<IconButton
					v-else-if="hasSwitchVersionListener && version && !hideSwitchVersion"
					v-tooltip="
						isDisabled && disabledTooltip
							? disabledTooltip
							: formatMessage(commonMessages.switchVersionButton)
					"
					type="quiet"
					:label="
						isDisabled && disabledTooltip
							? disabledTooltip
							: formatMessage(commonMessages.switchVersionButton)
					"
					:disabled="isDisabled"
					@click="emit('switchVersion')"
				>
					<ArrowLeftRightIcon class="size-5" />
				</IconButton>
			</div>

			<Toggle
				v-if="enabled !== undefined && !hideToggle"
				v-tooltip="
					isToggleDisabled && (toggleDisabledTooltip || disabledTooltip)
						? (toggleDisabledTooltip ?? disabledTooltip)
						: undefined
				"
				:model-value="enabled"
				:disabled="isToggleDisabled"
				:aria-label="project.title"
				class="my-auto"
				@update:model-value="(val) => emit('update:enabled', val as boolean)"
			/>

			<IconButton
				v-if="hasDeleteListener && !props.hideDelete"
				v-tooltip="
					isDisabled && disabledTooltip
						? disabledTooltip
						: formatMessage(
								shiftHeld && deleteHovered
									? commonMessages.deleteImmediatelyLabel
									: commonMessages.deleteLabel,
							)
				"
				type="quiet"
				:label="
					isDisabled && disabledTooltip
						? disabledTooltip
						: formatMessage(
								shiftHeld && deleteHovered
									? commonMessages.deleteImmediatelyLabel
									: commonMessages.deleteLabel,
							)
				"
				:disabled="isDisabled"
				@click="emit('delete', $event)"
				@mouseenter="deleteHovered = true"
				@mouseleave="deleteHovered = false"
			>
				<span class="relative size-5">
					<TrashIcon
						class="absolute inset-0 size-5 text-secondary transition-opacity duration-200"
						:class="shiftHeld && deleteHovered ? 'opacity-0' : 'opacity-100'"
					/>
					<TrashExclamationIcon
						class="absolute inset-0 size-5 text-red transition-opacity duration-200"
						:class="shiftHeld && deleteHovered ? 'opacity-100' : 'opacity-0'"
					/>
				</span>
			</IconButton>

			<slot name="additionalButtonsRight" />

			<TeleportOverflowMenu
				v-if="overflowOptions?.length"
				type="quiet"
				label="More options"
				:options="overflowOptions"
				:disabled="isDisabled"
			>
				<MoreVerticalIcon class="size-5" />
			</TeleportOverflowMenu>
		</div>
	</div>
</template>
