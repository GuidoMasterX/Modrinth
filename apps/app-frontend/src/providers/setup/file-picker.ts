import { provideFilePicker } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { readFile, stat } from '@tauri-apps/plugin-fs'

const MAX_PICKED_FILE_SIZE = 1024 * 1024 * 1024

async function readPickedFile(path: string) {
	const info = await stat(path)
	if (info.size > MAX_PICKED_FILE_SIZE) {
		throw new Error('The selected file is larger than the 1 GiB limit.')
	}
	return readFile(path)
}

function getFileName(path: string, fallback: string) {
	return path.split(/[\\/]/).pop() || fallback
}

function getDialogPath(result: string | { path?: string } | null | undefined) {
	if (!result) return null
	return typeof result === 'string' ? result : (result.path ?? null)
}

async function createFileFromPath(path: string, fallbackName: string, type?: string) {
	const bytes = await readPickedFile(path)
	const name = getFileName(path, fallbackName)
	return new File([bytes], name, type ? { type } : undefined)
}

async function createNativeFileFromPath(path: string, fallbackName: string, type?: string) {
	const bytes = await readPickedFile(path)
	const name = getFileName(path, fallbackName)
	return new File([bytes], name, type ? { type } : undefined)
}

export function setupFilePickerProvider() {
	provideFilePicker({
		async pickFiles(options) {
			const result = await open({
				multiple: options?.multiple ?? true,
			})
			if (!result) return []

			const paths = Array.isArray(result) ? result : [result]
			return await Promise.all(
				paths
					.map(getDialogPath)
					.filter((path): path is string => !!path)
					.map(async (path) => ({
						file: await createNativeFileFromPath(path, 'file'),
						path,
						previewUrl: convertFileSrc(path),
					})),
			)
		},
		async pickImage() {
			const result = await open({
				multiple: false,
				filters: [{ name: 'Image', extensions: ['png', 'jpeg', 'jpg', 'svg', 'webp', 'gif'] }],
			})
			if (!result) return null
			const path = getDialogPath(result)
			if (!path) return null
			const file = await createFileFromPath(path, 'icon')
			return { file, path, previewUrl: convertFileSrc(path) }
		},
		async pickModpackFile(options) {
			const result = await open({
				multiple: false,
				filters: [{ name: 'Modpack', extensions: ['mrpack', 'zip'] }],
			})
			if (!result) return null
			const path = getDialogPath(result)
			if (!path) return null
			if (options?.readFile === false) {
				// Instance imports stream from the native path, keeping large packs out of JS memory.
				return { path, previewUrl: '' }
			}
			return {
				file: await createFileFromPath(
					path,
					'modpack.mrpack',
					'application/x-modrinth-modpack+zip',
				),
				path,
				previewUrl: '',
			}
		},
	})
}
