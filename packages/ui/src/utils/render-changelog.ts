import { renderHighlightedString } from '@modrinth/utils'
import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'

const HTML_TAG_RE =
	/<(?:h[1-6]|ul|ol|li|p|div|a|img|table|thead|tbody|tr|td|th|blockquote|pre|code|strong|em|br|hr|span)\b/i
const MARKDOWN_RE =
	/(^|\n)\s{0,3}(#{1,6}\s|[-*+]\s|\d+\.\s|>\s|```|\|)|\*\*[^*\n]+\*\*|__[^_\n]+__|\[[^\]\n]+\]\([^)\n]+\)/

const markdown = new MarkdownIt({ html: true, linkify: true, breaks: true })

function sanitize(html: string) {
	return DOMPurify.sanitize(html, { ADD_ATTR: ['target', 'rel'] })
}

export function renderChangelog(changelog: string): string {
	if (HTML_TAG_RE.test(changelog)) return sanitize(changelog)
	if (MARKDOWN_RE.test(changelog)) return sanitize(markdown.render(changelog))
	return renderHighlightedString(changelog)
}
