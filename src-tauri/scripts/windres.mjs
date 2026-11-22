import fs from 'node:fs';
import path from 'node:path';

// Pure-Node Win32 RC -> COFF x86-64 (.rsrc) compiler for Tauri builds on gnullvm
// Supports ICON ("path") and 1 24 "manifest" (RT_MANIFEST), emitting a real COFF .rsrc object.

const args = process.argv.slice(2);
let inputRc = '';
let outputObj = '';
const includeDirs = [];

for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--input' || a === '-i') {
    inputRc = args[++i] || '';
  } else if (a === '--output' || a === '-o') {
    outputObj = args[++i] || '';
  } else if (a === '--include-dir' || a === '-I') {
    includeDirs.push(args[++i] || '');
  } else if (a === '-V' || a === '--version') {
    console.log('GNU windres (NetBoost built-in COFF resource compiler) 1.0');
    process.exit(0);
  }
}

if (!inputRc || !outputObj) {
  process.exit(0);
}

const rcDir = path.dirname(path.resolve(inputRc));
const rcText = fs.readFileSync(inputRc, 'utf8');

function resolveResourcePath(raw) {
  const unescaped = raw.replace(/\\\\/g, '\\');
  const candidates = [
    unescaped,
    path.resolve(rcDir, unescaped),
    ...includeDirs.map((d) => path.resolve(d, unescaped)),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return null;
}

// Resources organized by typeId -> Map<resId, Buffer>
// Win32 resource types: RT_ICON = 3, RT_GROUP_ICON = 14, RT_MANIFEST = 24
const resources = new Map();
function addRes(typeId, resId, buf) {
  if (!resources.has(typeId)) resources.set(typeId, new Map());
  resources.get(typeId).set(resId, buf);
}

// Parse 1 24 "manifest.xml" or 1 RT_MANIFEST "manifest.xml" or inline manifest
const manifestFileRe = /^\s*(\d+)\s+(?:24|RT_MANIFEST)\s+"([^"]+)"/gm;
let m;
while ((m = manifestFileRe.exec(rcText)) !== null) {
  const resId = Number.parseInt(m[1], 10);
  const p = resolveResourcePath(m[2]);
  if (p) {
    addRes(24, resId, fs.readFileSync(p));
  }
}

// Parse inline manifest block: 1 24 { "..." }
const inlineManifestRe = /^\s*(\d+)\s+(?:24|RT_MANIFEST)\s*\{([\s\S]*?)\}/gm;
while ((m = inlineManifestRe.exec(rcText)) !== null) {
  const resId = Number.parseInt(m[1], 10);
  const chunks = [];
  const strRe = /"((?:[^"\\]|\\.|"")*)"/g;
  let sm;
  while ((sm = strRe.exec(m[2])) !== null) {
    chunks.push(sm[1].replace(/""/g, '"').replace(/\\n/g, '\n').replace(/\\r/g, '\r').replace(/\\t/g, '\t').replace(/\\"/g, '"'));
  }
  if (chunks.length > 0) {
    addRes(24, resId, Buffer.from(chunks.join(''), 'utf8'));
  }
}

// Parse ICON lines: <id> ICON "path/to/icon.ico"
const iconRe = /^\s*(\d+)\s+ICON\s+"([^"]+)"/gm;
let nextIconImgId = 1;
while ((m = iconRe.exec(rcText)) !== null) {
  const groupId = Number.parseInt(m[1], 10);
  const p = resolveResourcePath(m[2]);
  if (!p) continue;
  const ico = fs.readFileSync(p);
  if (ico.length < 6) continue;
  const count = ico.readUInt16LE(4);
  if (count === 0 || ico.length < 6 + count * 16) continue;

  // Build GRPICONDIR (6 bytes + count * 14 bytes)
  const grp = Buffer.alloc(6 + count * 14);
  grp.writeUInt16LE(0, 0);
  grp.writeUInt16LE(1, 2); // type = icon
  grp.writeUInt16LE(count, 4);

  for (let idx = 0; idx < count; idx++) {
    const srcOff = 6 + idx * 16;
    const dstOff = 6 + idx * 14;
    const bytesInRes = ico.readUInt32LE(srcOff + 8);
    const imageOffset = ico.readUInt32LE(srcOff + 12);
    const iconId = nextIconImgId++;
    ico.copy(grp, dstOff, srcOff, srcOff + 12);
    grp.writeUInt16LE(iconId, dstOff + 12);
    if (imageOffset + bytesInRes <= ico.length) {
      addRes(3, iconId, ico.subarray(imageOffset, imageOffset + bytesInRes));
    }
  }
  addRes(14, groupId, grp);
}

// Build COFF x86-64 .rsrc object file
function align4(n) {
  return (n + 3) & ~3;
}

const typeIds = [...resources.keys()].sort((a, b) => a - b);

// Compute directory sizes
// Level 1: 1 directory (16 + typeIds.length * 8)
// Level 2: typeIds.length directories
// Level 3: totalEntries directories (each 1 lang entry: 16 + 8 = 24 bytes)
// Data entries: totalEntries * 16 bytes
// Raw data blobs: aligned to 4 bytes
let totalEntries = 0;
for (const tid of typeIds) totalEntries += resources.get(tid).size;

const lvl1Size = 16 + typeIds.length * 8;
let lvl2Offset = lvl1Size;
const lvl2Offsets = new Map();
for (const tid of typeIds) {
  lvl2Offsets.set(tid, lvl2Offset);
  lvl2Offset += 16 + resources.get(tid).size * 8;
}
const lvl3Start = lvl2Offset;
const dataEntryStart = lvl3Start + totalEntries * 24;
const dataBlobStart = dataEntryStart + totalEntries * 16;

let curBlobOffset = dataBlobStart;
const flatEntries = [];
for (const tid of typeIds) {
  const idMap = resources.get(tid);
  const resIds = [...idMap.keys()].sort((a, b) => a - b);
  for (const rid of resIds) {
    const buf = idMap.get(rid);
    const entryIndex = flatEntries.length;
    flatEntries.push({
      tid,
      rid,
      buf,
      lvl3Offset: lvl3Start + entryIndex * 24,
      dataEntryOffset: dataEntryStart + entryIndex * 16,
      blobOffset: curBlobOffset,
    });
    curBlobOffset = align4(curBlobOffset + buf.length);
  }
}

const rsrcSize = curBlobOffset;
const rsrcBuf = Buffer.alloc(rsrcSize);

// Write Level 1 directory
rsrcBuf.writeUInt16LE(typeIds.length, 14);
typeIds.forEach((tid, i) => {
  const off = 16 + i * 8;
  rsrcBuf.writeUInt32LE(tid, off);
  rsrcBuf.writeUInt32LE((0x80000000 | lvl2Offsets.get(tid)) >>> 0, off + 4);
});

// Write Level 2 directories
for (const tid of typeIds) {
  const dirOff = lvl2Offsets.get(tid);
  const items = flatEntries.filter((e) => e.tid === tid);
  rsrcBuf.writeUInt16LE(items.length, dirOff + 14);
  items.forEach((item, i) => {
    const off = dirOff + 16 + i * 8;
    rsrcBuf.writeUInt32LE(item.rid, off);
    rsrcBuf.writeUInt32LE((0x80000000 | item.lvl3Offset) >>> 0, off + 4);
  });
}

// Write Level 3 directories, Data entries, and Blobs + Relocations
const relocations = [];
for (const item of flatEntries) {
  // Level 3 directory (1 language: 0x0409 US English)
  rsrcBuf.writeUInt16LE(1, item.lvl3Offset + 14);
  rsrcBuf.writeUInt32LE(0x0409, item.lvl3Offset + 16);
  rsrcBuf.writeUInt32LE(item.dataEntryOffset, item.lvl3Offset + 20);

  // IMAGE_RESOURCE_DATA_ENTRY
  rsrcBuf.writeUInt32LE(item.blobOffset, item.dataEntryOffset);
  rsrcBuf.writeUInt32LE(item.buf.length, item.dataEntryOffset + 4);
  rsrcBuf.writeUInt32LE(65001, item.dataEntryOffset + 8); // UTF-8 codepage
  rsrcBuf.writeUInt32LE(0, item.dataEntryOffset + 12);

  // IMAGE_REL_AMD64_ADDR32NB (type = 3) against symbol 0 (.rsrc)
  relocations.push(item.dataEntryOffset);

  item.buf.copy(rsrcBuf, item.blobOffset);
}

// Assemble COFF object file
const coffHeaderSize = 20;
const secHeaderSize = 40;
const rawDataOffset = coffHeaderSize + secHeaderSize;
const relocOffset = rawDataOffset + rsrcSize;
const relocSize = relocations.length * 10;
const symTableOffset = relocOffset + relocSize;
const symTableSize = 18; // 1 symbol: .rsrc
const strTableSize = 4;

const coff = Buffer.alloc(symTableOffset + symTableSize + strTableSize);

// COFF Header
coff.writeUInt16LE(0x8664, 0); // IMAGE_FILE_MACHINE_AMD64
coff.writeUInt16LE(1, 2); // NumberOfSections
coff.writeUInt32LE(0, 4); // TimeDateStamp
coff.writeUInt32LE(symTableOffset, 8); // PointerToSymbolTable
coff.writeUInt32LE(1, 12); // NumberOfSymbols
coff.writeUInt16LE(0, 16); // SizeOfOptionalHeader
coff.writeUInt16LE(0x0004, 18); // IMAGE_FILE_LINE_NUMS_STRIPPED

// Section Header: .rsrc
coff.write('.rsrc', 20, 'ascii');
coff.writeUInt32LE(0, 28); // VirtualSize
coff.writeUInt32LE(0, 32); // VirtualAddress
coff.writeUInt32LE(rsrcSize, 36); // SizeOfRawData
coff.writeUInt32LE(rsrcSize > 0 ? rawDataOffset : 0, 40); // PointerToRawData
coff.writeUInt32LE(relocations.length > 0 ? relocOffset : 0, 44); // PointerToRelocations
coff.writeUInt32LE(0, 48); // PointerToLinenumbers
coff.writeUInt16LE(relocations.length, 52); // NumberOfRelocations
coff.writeUInt16LE(0, 54); // NumberOfLinenumbers
coff.writeUInt32LE(0x40000040, 56); // IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ

// Raw data
rsrcBuf.copy(coff, rawDataOffset);

// Relocations
relocations.forEach((va, idx) => {
  const off = relocOffset + idx * 10;
  coff.writeUInt32LE(va, off); // VirtualAddress
  coff.writeUInt32LE(0, off + 4); // SymbolTableIndex (.rsrc)
  coff.writeUInt16LE(3, off + 8); // IMAGE_REL_AMD64_ADDR32NB
});

// Symbol Table (1 symbol: .rsrc)
coff.write('.rsrc', symTableOffset, 'ascii');
coff.writeUInt32LE(0, symTableOffset + 8); // Value
coff.writeInt16LE(1, symTableOffset + 12); // SectionNumber (1-based)
coff.writeUInt16LE(0, symTableOffset + 14); // Type
coff.writeUInt8(3, symTableOffset + 16); // StorageClass = IMAGE_SYM_CLASS_STATIC
coff.writeUInt8(0, symTableOffset + 17); // NumberOfAuxSymbols

// String table size
coff.writeUInt32LE(4, symTableOffset + symTableSize);

fs.mkdirSync(path.dirname(path.resolve(outputObj)), { recursive: true });
fs.writeFileSync(outputObj, coff);
