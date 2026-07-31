# NEAR OutLayer Dashboard

Next.js dashboard for NEAR OutLayer off-chain computation platform.

## Setup Complete

✅ Next.js 15 + TypeScript + Tailwind CSS
✅ API client (lib/api.ts) 
✅ NEAR Wallet context (contexts/NearWalletContext.tsx)
✅ Layout with navigation
✅ .env.local configured

## To Implement

Create pages in app/:
- workers/page.tsx
- executions/page.tsx  
- stats/page.tsx
- playground/page.tsx
- settings/page.tsx

## Run

```bash
npm run dev
```

## llms.txt

`public/llms.txt` and `public/llms-full.txt` are **generated** — do not edit them by hand.
Edit `scripts/llms-manifest.mjs` (page summaries, link sections, which repo docs get inlined)
and regenerate:

```bash
npm run llms            # regenerate both files
npm run llms -- --strict  # fail if a manifest entry no longer exists — use in CI
```

`predev` and `prebuild` run this automatically, so the files stay in sync with the repo docs.
