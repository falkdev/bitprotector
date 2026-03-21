# BitProtector Frontend

React + TypeScript + Vite frontend for the BitProtector web UI.

## Requirements

- Node.js 20 or newer

## Commands

```bash
npm ci
npm run dev
npm run build
npm run lint
npm test
```

## Notes

- Development uses the Vite proxy for `/api/*` requests.
- Production assets are built into `dist/`.
- The Rust backend serves the built frontend from `/var/lib/bitprotector/frontend` on the same origin as `/api/v1`.
- To test locally against the manual QEMU backend, run `../scripts/frontend_qemu_manual.sh`.
