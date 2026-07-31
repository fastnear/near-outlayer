#!/usr/bin/env node
// Generates public/llms.txt and public/llms-full.txt from scripts/llms-manifest.mjs.
//
// Run automatically by `npm run dev` and `npm run build` (see predev/prebuild).
// Pass --strict to fail on a missing source file instead of warning — use that in CI.

import { statSync } from 'node:fs'
import { readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { posix } from 'node:path'
import { fileURLToPath } from 'node:url'

import { FULL_TEXT_SOURCES, SECTIONS, SITE } from './llms-manifest.mjs'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const dashboardDir = resolve(scriptDir, '..')
const repoRoot = resolve(dashboardDir, '..')
const publicDir = resolve(dashboardDir, 'public')

const strict = process.argv.includes('--strict')

/** Absolute URL for a manifest entry: either an explicit `url` or a site-relative `path`. */
function entryUrl(entry) {
  return entry.url ?? `${SITE.baseUrl}${entry.path}`
}

function renderLinkList(pages) {
  return pages.map((p) => `- [${p.title}](${entryUrl(p)}): ${p.summary}`).join('\n')
}

function buildIndex() {
  const parts = [`# ${SITE.name}`, '', `> ${SITE.summary}`, '', SITE.details.join('\n\n')]

  for (const section of SECTIONS) {
    parts.push('', `## ${section.title}`, '', renderLinkList(section.pages))
  }

  return `${parts.join('\n')}\n`
}

/**
 * Rewrites repository-relative markdown links to absolute GitHub URLs.
 * Inlined docs lose their directory context, so `[CLI](docs/CLI.md)` would otherwise dangle.
 *
 * Only targets that actually exist in this repository are rewritten — several docs link into
 * the private coordinator repo, and turning those into public URLs would fabricate dead links.
 */
function absolutizeLinks(markdown, sourcePath) {
  const sourceDir = posix.dirname(sourcePath)

  return markdown.replace(/\]\(([^)\s]+)(\s+"[^"]*")?\)/g, (match, target, title = '') => {
    if (/^(https?:|mailto:|#|\/)/.test(target)) return match

    const [pathPart, anchor = ''] = target.split(/(#.*)$/)
    if (!pathPart) return match

    const resolved = posix.normalize(posix.join(sourceDir, pathPart))
    if (resolved.startsWith('..')) return match
    if (resolved === '.') return `](${SITE.repoUrl}${anchor}${title})`

    let stats
    try {
      stats = statSync(resolve(repoRoot, resolved))
    } catch {
      return match
    }

    const kind = stats.isDirectory() ? 'tree' : 'blob'
    return `](${SITE.repoUrl}/${kind}/main/${resolved}${anchor}${title})`
  })
}

async function loadSources() {
  const loaded = []
  const missing = []

  for (const source of FULL_TEXT_SOURCES) {
    try {
      const raw = await readFile(resolve(repoRoot, source.path), 'utf8')
      loaded.push({ ...source, body: absolutizeLinks(raw.trim(), source.path) })
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
      missing.push(source.path)
    }
  }

  return { loaded, missing }
}

function buildFullText(sources) {
  const toc = sources
    .map((s) => `- ${s.title} — \`${s.path}\``)
    .join('\n')

  const parts = [
    `# ${SITE.name} — Full Documentation`,
    '',
    `> ${SITE.summary}`,
    '',
    SITE.details.join('\n\n'),
    '',
    'This file inlines every developer-facing document in the OutLayer repository.',
    `For a link index instead, fetch ${SITE.baseUrl}/llms.txt.`,
    '',
    '## Contents',
    '',
    toc,
  ]

  for (const source of sources) {
    parts.push(
      '',
      '---',
      '',
      `<!-- source: ${source.path} -->`,
      `> Source: ${SITE.repoUrl}/blob/main/${source.path}`,
      '',
      source.body,
    )
  }

  return `${parts.join('\n')}\n`
}

async function main() {
  const { loaded, missing } = await loadSources()

  if (missing.length > 0) {
    const message =
      `llms: ${missing.length} source file(s) listed in scripts/llms-manifest.mjs are missing:\n` +
      missing.map((p) => `  - ${p}`).join('\n')

    if (strict) throw new Error(message)
    console.warn(`${message}\nUpdate the manifest — these docs are absent from llms-full.txt.`)
  }

  if (loaded.length === 0) {
    throw new Error(
      `llms: no source documents resolved under ${repoRoot}. ` +
        'llms-full.txt would be empty, refusing to write.',
    )
  }

  const index = buildIndex()
  const full = buildFullText(loaded)

  await writeFile(resolve(publicDir, 'llms.txt'), index, 'utf8')
  await writeFile(resolve(publicDir, 'llms-full.txt'), full, 'utf8')

  const kb = (text) => `${Math.round(Buffer.byteLength(text) / 1024)} KB`
  const linkCount = SECTIONS.reduce((n, s) => n + s.pages.length, 0)
  console.log(
    `llms: wrote public/llms.txt (${linkCount} links, ${kb(index)}) ` +
      `and public/llms-full.txt (${loaded.length} documents, ${kb(full)})`,
  )
}

main().catch((error) => {
  console.error(error.message)
  process.exit(1)
})
