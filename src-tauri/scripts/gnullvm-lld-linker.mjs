import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const home = process.env.USERPROFILE || os.homedir();
const gnullvm = path.join(
  home,
  '.rustup',
  'toolchains',
  'stable-x86_64-pc-windows-gnullvm',
  'lib',
  'rustlib',
  'x86_64-pc-windows-gnullvm'
);
const gnu = path.join(
  home,
  '.rustup',
  'toolchains',
  'stable-x86_64-pc-windows-gnu',
  'lib',
  'rustlib',
  'x86_64-pc-windows-gnu'
);
const lld = path.join(gnullvm, 'bin', 'gcc-ld', 'ld.lld.exe');

function parseRsp(content) {
  const tokens = [];
  let cur = '';
  let inQuote = false;
  let quoteChar = '';
  let hasToken = false;
  for (let i = 0; i < content.length; i++) {
    const ch = content[i];
    if (inQuote) {
      if (ch === '\\' && i + 1 < content.length && (content[i + 1] === '"' || content[i + 1] === '\\')) {
        cur += content[++i];
      } else if (ch === quoteChar) {
        inQuote = false;
      } else {
        cur += ch;
      }
    } else if (ch === '\\' && i + 1 < content.length && !/[\r\n]/.test(content[i + 1])) {
      cur += content[++i];
      hasToken = true;
    } else if (ch === '"' || ch === "'") {
      inQuote = true;
      quoteChar = ch;
      hasToken = true;
    } else if (/\s/.test(ch)) {
      if (hasToken) {
        tokens.push(cur);
        cur = '';
        hasToken = false;
      }
    } else {
      cur += ch;
      hasToken = true;
    }
  }
  if (hasToken) tokens.push(cur);
  return tokens;
}

function expandArgs(argv) {
  const out = [];
  for (const a of argv) {
    if (a.startsWith('@') && a.length > 1) {
      const rspPath = a.slice(1);
      if (fs.existsSync(rspPath)) {
        const buf = fs.readFileSync(rspPath);
        const text =
          buf.length >= 2 && buf[0] === 0xff && buf[1] === 0xfe
            ? buf.slice(2).toString('utf16le')
            : buf.toString('utf8');
        out.push(...parseRsp(text));
        continue;
      }
    }
    out.push(a);
  }
  return out;
}

const raw = expandArgs(process.argv.slice(2));
const isShared = raw.includes('-shared');

const lldArgs = [
  '-m',
  'i386pep',
  isShared ? '--dll' : '-Bdynamic',
  isShared
    ? path.join(gnullvm, 'lib', 'self-contained', 'dllcrt2.o')
    : path.join(gnullvm, 'lib', 'self-contained', 'crt2.o'),
  '-L' + path.join(gnullvm, 'lib', 'self-contained'),
  '-L' + path.join(gnu, 'lib', 'self-contained'),
];

if (isShared) {
  lldArgs.push('-e', 'DllMainCRTStartup');
}

for (let i = 0; i < raw.length; i++) {
  const a = raw[i];
  if (
    a === '-nolibc' ||
    a === '--unwindlib=libunwind' ||
    a === '-static-libgcc' ||
    a === '-m64' ||
    a === '-shared' ||
    a === '-no-pie' ||
    a === '-pie' ||
    a.startsWith('-fuse-ld=')
  ) {
    continue;
  }
  if (a === '-mwindows') {
    lldArgs.push('--subsystem', 'windows');
    continue;
  }
  if (a === '-mconsole') {
    lldArgs.push('--subsystem', 'console');
    continue;
  }
  if (a.startsWith('-Wl,')) {
    const parts = a.slice(4).split(',');
    lldArgs.push(...parts);
    continue;
  }
  lldArgs.push(a);
}

lldArgs.push(
  '--start-group',
  '-lmingw32',
  '-lmingwex',
  '-lmsvcrt',
  '-lkernel32',
  '-lunwind',
  '--end-group'
);

const rspFile = path.join(os.tmpdir(), `lld-rsp-${process.pid}-${crypto.randomBytes(6).toString('hex')}.rsp`);
const rspBody = lldArgs
  .map((arg) => {
    const escaped = arg.replace(/\\/g, '/').replace(/"/g, '\\"');
    return /[\s"]/u.test(escaped) ? `"${escaped}"` : escaped;
  })
  .join('\n');

fs.writeFileSync(rspFile, rspBody, 'utf8');
try {
  const res = spawnSync(lld, [`@${rspFile}`], { stdio: 'inherit' });
  if (res.error) {
    console.error('gnullvm-lld-linker spawn error:', res.error);
    process.exit(1);
  }
  process.exit(res.status ?? 1);
} finally {
  try {
    fs.unlinkSync(rspFile);
  } catch {
    // ignore cleanup error
  }
}
