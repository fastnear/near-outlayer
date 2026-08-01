#!/usr/bin/env node
// Generates public/llms.txt and public/llms-full.txt from scripts/llms-manifest.mjs.
//
// Run automatically by `npm run dev` and `npm run build` (see predev/prebuild).
// Pass --strict to fail on a missing source file instead of warning — use that in CI.

import { readFileSync, statSync } from 'node:fs'
import { readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { posix } from 'node:path'
import { fileURLToPath } from 'node:url'

import { FULL_TEXT_SOURCES, SECTIONS, SITE } from './llms-manifest.mjs'

/**
 * Submodule path -> the URL of the repository it lives in.
 *
 * Needed because a submodule is a gitlink, not a directory: GitHub cannot render
 * `<monorepo>/tree/main/wasi-examples/oracle-example`, it 404s. Every example under
 * wasi-examples/ is a submodule, so without this every example link in llms-full.txt is dead.
 */
function readSubmodules() {
  const map = new Map()
  let gitmodules
  try {
    gitmodules = readFileSync(resolve(repoRoot, '.gitmodules'), 'utf8')
  } catch {
    return map
  }
  let path = null
  for (const line of gitmodules.split('\n')) {
    const p = line.match(/^\s*path\s*=\s*(\S+)/)
    if (p) path = p[1]
    const u = line.match(/^\s*url\s*=\s*(\S+)/)
    if (u && path) {
      const url = u[1]
        .replace(/^git@github\.com:/, 'https://github.com/')
        .replace(/\.git$/, '')
        .replace(/\/$/, '')
      map.set(path, url)
      path = null
    }
  }
  return map
}

/** If `target` is a submodule or a path inside one, the URL that actually resolves. */
function submoduleUrl(target) {
  for (const [path, url] of SUBMODULES) {
    if (target === path) return { url, rest: '' }
    if (target.startsWith(path + '/')) return { url, rest: target.slice(path.length + 1) }
  }
  return null
}

const scriptDir = dirname(fileURLToPath(import.meta.url))
const dashboardDir = resolve(scriptDir, '..')
const repoRoot = resolve(dashboardDir, '..')

// Declared after repoRoot on purpose: readSubmodules() needs it. An empty map here would
// silently send every example link back to a 404, so it is loud rather than best-effort.
const SUBMODULES = readSubmodules()
if (SUBMODULES.size === 0) {
  console.warn('llms: no submodules found in .gitmodules — example links will not resolve')
}
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

    const sub = submoduleUrl(resolved.replace(/\/$/, ''))
    if (sub) {
      if (!sub.rest) return `](${sub.url}${anchor}${title})`
      let subKind = 'blob'
      try {
        if (statSync(resolve(repoRoot, resolved)).isDirectory()) subKind = 'tree'
      } catch {
        return match
      }
      return `](${sub.url}/${subKind}/main/${sub.rest}${anchor}${title})`
    }

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
