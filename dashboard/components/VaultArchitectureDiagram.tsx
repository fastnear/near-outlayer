'use client';

import { useState } from 'react';

/**
 * Interactive architecture diagram for /docs/vaults.
 *
 * Top-down flow following a key-derivation request from the
 * customer's end-user / sub-agent down to NEAR MPC's threshold
 * derivation. The API edge is intentionally not its own box — what
 * matters is how the key gets derived.
 *
 *   1. End user / sub-agent (top) — your app's user, the OAuth
 *      account you minted for them, the autonomous agent you spawned.
 *   2. Runtime / TEE — holds the per-customer master in enclave
 *      memory and HKDF-derives the requested key. Color and label
 *      change per mode.
 *   3. CKD issuer contract — predecessor account that binds the
 *      master to MPC. Default = OutLayer's keystore-DAO; vault =
 *      your contract.
 *   4. NEAR MPC network (bottom) — threshold-key root.
 *
 * Visual style: muted, design-system-friendly. White cards, 2px
 * colored borders, monochrome stroke icons. Smooth color
 * transitions when the user flips between modes. Layout is tight
 * with no vertical gaps between arrows and cards.
 */
type Mode = 'default' | 'vault-managed' | 'vault-self';

const ACCENT = {
  user: '#6366f1',     // indigo — the trigger, mode-agnostic
  outlayer: '#ea580c', // orange-700
  vault: '#2563eb',    // blue-600
  self: '#15803d',     // green-700
  mpc: '#7c3aed',      // violet-600
  arrow: '#9ca3af',
};

const MODE_CONFIG: Record<
  Mode,
  {
    runtimeTitle: string;
    runtimeSub: string;
    runtimeAccent: string;
    runtimeChip: string;          // who operates the runtime
    runtimeChipColor: string;
    runtimeArrow: string;
    issuerTitle: string;
    issuerSub: string;
    issuerAccent: string;
    issuerChip: string;           // master scope
    issuerChipColor: string;
    issuerArrow: string;
    badge: { icon: string; text: string; bg: string; border: string; fg: string };
  }
> = {
  default: {
    runtimeTitle: 'OutLayer keystore TEE',
    runtimeSub:
      'Intel TDX enclave, attested by DAO. HKDF-derives the requested key from a master held in enclave memory.',
    runtimeAccent: ACCENT.outlayer,
    runtimeChip: 'OutLayer',
    runtimeChipColor: ACCENT.outlayer,
    runtimeArrow:
      'first call only — keystore TEE asks MPC bound to keystore-DAO',
    issuerTitle: 'OutLayer keystore-DAO contract',
    issuerSub: 'shared CKD root — every default-master customer derives from the same master',
    issuerAccent: ACCENT.outlayer,
    issuerChip: 'OutLayer',
    issuerChipColor: ACCENT.outlayer,
    issuerArrow: 'CKD bound to keystore-DAO',
    badge: {
      icon: '·',
      text: 'CKD-as-a-service. Zero setup, no on-chain footprint per customer. Stays bound to OutLayer — pick this for prototyping or anything that lives entirely on OutLayer.',
      bg: '#f9fafb',
      border: '#e5e7eb',
      fg: '#374151',
    },
  },
  'vault-managed': {
    runtimeTitle: 'OutLayer keystore TEE',
    runtimeSub:
      'currently in the slot. Replaceable: a future recovery puts your own attested runtime here.',
    runtimeAccent: ACCENT.outlayer,
    runtimeChip: 'OutLayer',
    runtimeChipColor: ACCENT.outlayer,
    runtimeArrow:
      'first call only — keystore TEE asks MPC via vault.request_master()',
    issuerTitle: 'Your MPC-vault contract',
    issuerSub: 'sub-account of your NEAR account; on-chain CKD issuer',
    issuerAccent: ACCENT.vault,
    issuerChip: 'Yours',
    issuerChipColor: ACCENT.vault,
    issuerArrow: 'CKD bound to your vault contract',
    badge: {
      icon: '↺',
      text: 'Takeover available. Initiate recovery (cessation or unilateral exit) to swap the runtime for your own; same vault, same MPC, same keys.',
      bg: '#eff6ff',
      border: '#bfdbfe',
      fg: '#1e40af',
    },
  },
  'vault-self': {
    runtimeTitle: 'Your TEE / runtime',
    runtimeSub:
      'attested box you brought after recovery. Same vault binding, same MPC, identical derived bytes.',
    runtimeAccent: ACCENT.self,
    runtimeChip: 'You',
    runtimeChipColor: ACCENT.self,
    runtimeArrow:
      'first call only — your runtime asks MPC via vault.request_master()',
    issuerTitle: 'Your MPC-vault contract',
    issuerSub: 'unchanged — same on-chain CKD issuer',
    issuerAccent: ACCENT.vault,
    issuerChip: 'Yours',
    issuerChipColor: ACCENT.vault,
    issuerArrow: 'CKD bound to your vault contract',
    badge: {
      icon: '✓',
      text: 'Self-managed. OutLayer is no longer in the loop. Your end-users keep getting byte-identical keys because the vault binding is yours.',
      bg: '#f0fdf4',
      border: '#bbf7d0',
      fg: '#166534',
    },
  },
};

// ─── Layout (single source of truth for vertical positions) ─────────────
//
// Tight stack: each card's bottom is exactly the next arrow's start.
// No gaps. Numbers are y-coords inside the SVG viewBox.

const L = {
  card1: { y: 10, h: 88 },     // user
  arrow1: { y1: 98, y2: 158 }, // 60 px tall arrow
  card2: { y: 158, h: 116 },   // runtime
  arrow2: { y1: 274, y2: 334 },
  card3: { y: 334, h: 78 },    // issuer
  arrow3: { y1: 412, y2: 472 },
  card4: { y: 472, h: 78 },    // MPC
  fanOut: { y1: 550, fanY: 600 }, // trunk goes straight to horizontal spine
  useCases: { y: 612, h: 70 },    // chip top sits 12 px below the spine
};

const VIEW_HEIGHT = L.useCases.y + L.useCases.h + 12;

// Concrete things the master is used to derive on demand. Mode-
// agnostic — these are the actual customer-facing keys / artefacts
// that come out of the pipeline above. Each is a leaf of the dashed
// fan-out under the NEAR MPC card.
const USE_CASES: { title: string; sub: string }[] = [
  {
    title: 'End-user wallets',
    sub: 'Crosschain accounts per OAuth user',
  },
  {
    title: 'Agent wallets',
    sub: 'Per-agent / sub-agent keypairs',
  },
  {
    title: 'Encrypted secrets',
    sub: 'API keys, env vars; TEE-only',
  },
  {
    title: 'On-demand signatures',
    sub: 'Sign for any derived address',
  },
];

export function VaultArchitectureDiagram() {
  const [mode, setMode] = useState<Mode>('default');
  const cfg = MODE_CONFIG[mode];
  const t = 'transition-all duration-500 ease-in-out';

  return (
    <div className="border-2 border-gray-200 rounded-lg p-4 mb-6 bg-gray-50">
      {/* Mode tabs */}
      <div className="flex flex-col sm:flex-row gap-1 mb-4 border border-gray-200 rounded p-1 bg-white">
        <ModeButton current={mode} value="default" set={setMode}>
          Default (shared OutLayer)
        </ModeButton>
        <ModeButton current={mode} value="vault-managed" set={setMode}>
          MPC vault &mdash; OutLayer-managed
        </ModeButton>
        <ModeButton current={mode} value="vault-self" set={setMode}>
          MPC vault &mdash; self-managed
        </ModeButton>
      </div>

      {/* SVG diagram */}
      <div className="bg-white border border-gray-200 rounded p-2 overflow-x-auto">
        <svg
          viewBox={`0 0 760 ${VIEW_HEIGHT}`}
          className="w-full"
          style={{ maxWidth: '760px', margin: '0 auto', display: 'block' }}
        >
          <defs>
            <marker
              id="vaultArrow"
              markerWidth="10"
              markerHeight="10"
              refX="9"
              refY="3"
              orient="auto"
              markerUnits="strokeWidth"
            >
              <path d="M0,0 L0,6 L9,3 z" fill={ACCENT.arrow} />
            </marker>
          </defs>

          {/* 1. End user / sub-agent */}
          <Card
            x={140}
            y={L.card1.y}
            width={480}
            height={L.card1.h}
            accent={ACCENT.user}
            icon={<UserIcon />}
            title="Your end users / sub-agents"
            subLines={[
              'OAuth users you mint a NEAR account for, autonomous sub-agents,',
              'any account you control on a customer’s behalf.',
            ]}
            t={t}
          />

          <FlowArrow
            x={380}
            y1={L.arrow1.y1}
            y2={L.arrow1.y2}
            label='"sign this tx", "decrypt my secret", "give me my address"'
          />

          {/* 2. Runtime / TEE */}
          <Card
            x={140}
            y={L.card2.y}
            width={480}
            height={L.card2.h}
            accent={cfg.runtimeAccent}
            icon={<ShieldIcon />}
            title={cfg.runtimeTitle}
            subLines={[
              cfg.runtimeSub,
              'Derives requested key on the fly: secret = HMAC-SHA256(master, seed).',
            ]}
            chip={cfg.runtimeChip}
            chipColor={cfg.runtimeChipColor}
            t={t}
          />

          <FlowArrow
            x={380}
            y1={L.arrow2.y1}
            y2={L.arrow2.y2}
            label={cfg.runtimeArrow}
          />

          {/* 3. CKD issuer contract */}
          <Card
            x={140}
            y={L.card3.y}
            width={480}
            height={L.card3.h}
            accent={cfg.issuerAccent}
            icon={<DocumentIcon />}
            title={cfg.issuerTitle}
            subLines={[cfg.issuerSub]}
            chip={cfg.issuerChip}
            chipColor={cfg.issuerChipColor}
            t={t}
          />

          <FlowArrow
            x={380}
            y1={L.arrow3.y1}
            y2={L.arrow3.y2}
            label={cfg.issuerArrow}
            sub="DAO contract gates MPC on TEE hardware-attestation verification"
          />

          {/* 4. NEAR MPC network */}
          <Card
            x={140}
            y={L.card4.y}
            width={480}
            height={L.card4.h}
            accent={ACCENT.mpc}
            icon={<NetworkIcon />}
            title="NEAR MPC network"
            subLines={[
              'threshold key holders return a 32-byte master per CKD path',
            ]}
            t={t}
          />

          {/* 5. Fan-out: master → concrete use cases (mode-agnostic) */}
          <UseCaseFanOut
            y1={L.fanOut.y1}
            fanY={L.fanOut.fanY}
            chipY={L.useCases.y}
            chipH={L.useCases.h}
            cases={USE_CASES}
          />
        </svg>
      </div>

      {/* Takeover badge — varies per mode, sits below the outcome block */}
      <div
        className={`mt-3 px-4 py-3 rounded border text-sm flex items-start gap-3 ${t}`}
        style={{
          backgroundColor: cfg.badge.bg,
          borderColor: cfg.badge.border,
          color: cfg.badge.fg,
        }}
      >
        <span className="text-base font-bold leading-none mt-0.5">{cfg.badge.icon}</span>
        <span>{cfg.badge.text}</span>
      </div>

      <p className="text-xs text-gray-500 mt-3 italic">
        Read top-down: end-user triggers a request &rarr; the runtime (TEE)
        derives the requested key from a per-customer master &rarr; on first
        call only, the master comes from NEAR MPC bound to a CKD-issuer
        contract. Tabs change two things: which contract owns the binding
        and who operates the runtime &mdash; watch the chips on the runtime
        and issuer cards flip.
      </p>
    </div>
  );
}

// ─── Use-case fan-out under MPC ────────────────────────────────────────────
//
// Renders a dashed branch coming off the bottom of the NEAR MPC card and
// fanning out to N small "what gets derived" chips. Mode-agnostic — the
// same set of customer-facing artefacts is derivable in every mode; the
// only difference between modes is whose master they hang off.

function UseCaseFanOut({
  y1,
  fanY,
  chipY,
  chipH,
  cases,
}: {
  y1: number;
  fanY: number;
  chipY: number;
  chipH: number;
  cases: { title: string; sub: string }[];
}) {
  // Chip layout: distribute across the 760-px viewBox with equal gaps.
  const viewW = 760;
  const sideMargin = 22;
  const gap = 10;
  const usable = viewW - sideMargin * 2 - gap * (cases.length - 1);
  const chipW = Math.floor(usable / cases.length);
  const stroke = '#9ca3af';
  const dash = '4,4';

  return (
    <g>
      {/* Trunk: dashed line straight down from MPC bottom to the spine */}
      <line
        x1={viewW / 2}
        y1={y1}
        x2={viewW / 2}
        y2={fanY}
        stroke={stroke}
        strokeWidth="1.5"
        strokeDasharray={dash}
      />
      <text
        x={viewW / 2 + 12}
        y={(y1 + fanY) / 2 + 3}
        fontSize="10"
        fill="#6b7280"
      >
        derived on demand inside TEE — never stored
      </text>

      {/* Horizontal spine */}
      {cases.length > 1 && (
        <line
          x1={sideMargin + chipW / 2}
          y1={fanY}
          x2={viewW - sideMargin - chipW / 2}
          y2={fanY}
          stroke={stroke}
          strokeWidth="1.5"
          strokeDasharray={dash}
        />
      )}

      {/* Per-chip drop + chip */}
      {cases.map((c, i) => {
        const cx = sideMargin + i * (chipW + gap) + chipW / 2;
        const x = sideMargin + i * (chipW + gap);
        return (
          <g key={i}>
            {/* Drop from spine to chip top */}
            <line
              x1={cx}
              y1={fanY}
              x2={cx}
              y2={chipY}
              stroke={stroke}
              strokeWidth="1.5"
              strokeDasharray={dash}
            />
            {/* Chip body */}
            <rect
              x={x}
              y={chipY}
              width={chipW}
              height={chipH}
              rx={8}
              fill="#ffffff"
              stroke="#d1d5db"
              strokeWidth="1.25"
            />
            {/* Title */}
            <text
              x={cx}
              y={chipY + 24}
              textAnchor="middle"
              fontSize="11.5"
              fontWeight="700"
              fill="#111827"
            >
              {truncateForCard(c.title, chipW - 12)}
            </text>
            {/* Sub-line: split into <=2 lines if it doesn't fit one */}
            {wrapLines(c.sub, chipW - 12, 2).map((line, li) => (
              <text
                key={li}
                x={cx}
                y={chipY + 42 + li * 13}
                textAnchor="middle"
                fontSize="10"
                fill="#4b5563"
              >
                {line}
              </text>
            ))}
          </g>
        );
      })}
    </g>
  );
}

// Crude one-line wrapper for SVG text — splits on word boundaries up to
// `maxLines` and ellipsises the tail if everything won't fit. Used for
// the fan-out chips below MPC where we have ~110 px of width.
function wrapLines(s: string, widthPx: number, maxLines: number): string[] {
  const charsPerLine = Math.floor(widthPx / 5.4);
  if (s.length <= charsPerLine) return [s];
  const words = s.split(/\s+/);
  const lines: string[] = [];
  let cur = '';
  for (const w of words) {
    const cand = cur ? `${cur} ${w}` : w;
    if (cand.length > charsPerLine) {
      if (cur) lines.push(cur);
      cur = w;
      if (lines.length >= maxLines) break;
    } else {
      cur = cand;
    }
  }
  if (cur && lines.length < maxLines) lines.push(cur);
  if (lines.length === maxLines) {
    // tail might still overflow — ellipsise
    const last = lines[maxLines - 1];
    if (last.length > charsPerLine) {
      lines[maxLines - 1] = last.slice(0, charsPerLine - 1).trimEnd() + '…';
    }
  }
  return lines;
}

// ─── Card primitive ────────────────────────────────────────────────────────
//
// White card, 2 px colored border, 6 px colored accent stripe on the
// left, soft-tinted icon square. The chip floats top-right with a
// subtle tint so it doesn't fight the title for attention.

type CardProps = {
  x: number;
  y: number;
  width: number;
  height: number;
  accent: string;
  icon: React.ReactNode;
  title: string;
  subLines: string[];
  chip?: string;
  chipColor?: string;
  t: string;
};

function Card({ x, y, width, height, accent, icon, title, subLines, chip, chipColor, t }: CardProps) {
  const iconSize = 26;
  const iconBoxX = x + 14;
  const iconBoxY = y + 14;
  const titleX = iconBoxX + iconSize + 12;
  const titleY = y + 28;
  const subStartY = y + 50;

  return (
    <g>
      {/* Card body */}
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        rx={10}
        fill="#ffffff"
        stroke={accent}
        strokeWidth="2"
        className={t}
      />
      {/* Left accent stripe */}
      <rect
        x={x}
        y={y}
        width={6}
        height={height}
        rx={3}
        fill={accent}
        className={t}
      />
      {/* Icon in soft-tinted square */}
      <g transform={`translate(${iconBoxX}, ${iconBoxY})`}>
        <rect
          width={iconSize}
          height={iconSize}
          rx={6}
          fill={accent}
          opacity={0.12}
          className={t}
        />
        <g transform={`translate(${(iconSize - 18) / 2}, ${(iconSize - 18) / 2})`}>
          <g style={{ color: accent }} className={t}>
            {icon}
          </g>
        </g>
      </g>

      {/* Title */}
      <text x={titleX} y={titleY} fontSize="14" fontWeight="700" fill="#111827">
        {title}
      </text>

      {/* Chip top-right */}
      {chip && chipColor && (
        <g>
          <rect
            x={x + width - 130}
            y={y + 14}
            width={120}
            height={20}
            rx={10}
            fill={chipColor}
            opacity={0.12}
            className={t}
          />
          <rect
            x={x + width - 130}
            y={y + 14}
            width={120}
            height={20}
            rx={10}
            fill="none"
            stroke={chipColor}
            strokeWidth="1"
            className={t}
          />
          <text
            x={x + width - 70}
            y={y + 28}
            textAnchor="middle"
            fontSize="10"
            fontWeight="700"
            fill={chipColor}
            className={t}
          >
            {chip}
          </text>
        </g>
      )}

      {/* Sub-lines (auto-truncated to fit width) */}
      {subLines.map((line, i) => (
        <text
          key={i}
          x={titleX}
          y={subStartY + i * 18}
          fontSize="11"
          fill="#4b5563"
        >
          {truncateForCard(line, width - (titleX - x) - 14)}
        </text>
      ))}
    </g>
  );
}

// SVG <text> doesn't auto-wrap and overflows the card. We allot
// ~5.6 px per character at 11 px Inter / system-ui and cut with an
// ellipsis. Subtitles stay readable; if they need more, the takeover
// badge below the diagram carries the full prose.
function truncateForCard(s: string, widthPx: number): string {
  const maxChars = Math.floor(widthPx / 5.6);
  if (s.length <= maxChars) return s;
  return s.slice(0, maxChars - 1).trimEnd() + '…';
}

// ─── Arrow ─────────────────────────────────────────────────────────────────

function FlowArrow({
  x,
  y1,
  y2,
  label,
  sub,
}: {
  x: number;
  y1: number;
  y2: number;
  label: string;
  sub?: string;
}) {
  const midY = (y1 + y2) / 2;
  return (
    <g>
      <line
        x1={x}
        y1={y1 + 2}
        x2={x}
        y2={y2 - 2}
        stroke={ACCENT.arrow}
        strokeWidth="2"
        markerEnd="url(#vaultArrow)"
      />
      <text
        x={x + 12}
        y={midY - (sub ? 4 : 2)}
        fontSize="11"
        fontWeight="600"
        fill="#374151"
      >
        {label}
      </text>
      {sub && (
        <text x={x + 12} y={midY + 12} fontSize="10" fill="#6b7280">
          {sub}
        </text>
      )}
    </g>
  );
}

// ─── Tab button ────────────────────────────────────────────────────────────

function ModeButton({
  current,
  value,
  set,
  children,
}: {
  current: Mode;
  value: Mode;
  set: (v: Mode) => void;
  children: React.ReactNode;
}) {
  const active = current === value;
  return (
    <button
      type="button"
      onClick={() => set(value)}
      aria-pressed={active}
      className={`flex-1 px-3 py-2 rounded text-sm font-medium transition-colors ${
        active
          ? 'bg-gray-900 text-white shadow-sm'
          : 'text-gray-700 hover:bg-gray-100'
      }`}
    >
      {children}
    </button>
  );
}

// ─── Restrained monochrome icons (18×18, stroke-only) ──────────────────────

function UserIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="9" cy="6" r="3" />
      <path d="M3 16c0-3 2.7-5 6-5s6 2 6 5" strokeLinecap="round" />
    </svg>
  );
}

function ShieldIcon() {
  // Lock + shield = TEE / enclave
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6">
      <path d="M9 1.5l6 2v5c0 4-3 6.5-6 7.5-3-1-6-3.5-6-7.5v-5l6-2z" strokeLinejoin="round" />
      <rect x="6.5" y="7" width="5" height="4" rx="0.5" />
      <path d="M7.5 7V5.5a1.5 1.5 0 013 0V7" />
    </svg>
  );
}

function DocumentIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6">
      <path d="M3 2h7l4 4v10H3V2z" strokeLinejoin="round" />
      <path d="M10 2v4h4" strokeLinejoin="round" />
      <path d="M5.5 10h7M5.5 13h7M5.5 7h2.5" strokeLinecap="round" />
    </svg>
  );
}

function NetworkIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="9" cy="3" r="1.6" />
      <circle cx="3" cy="14" r="1.6" />
      <circle cx="15" cy="14" r="1.6" />
      <path d="M9 4.5L4 12.5M9 4.5l5 8M4.5 14h9" />
    </svg>
  );
}
