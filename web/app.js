/**
 * bdd Web UI Client Application
 * Handles live pattern parsing, preset loading, file drag-and-drop,
 * dynamic CLI command synthesis, and real-time execution via /api/process.
 */

const PRESETS = [
  {
    id: "mp3-header",
    name: "MP3 Header",
    icon: "🎵",
    category: "multimedia",
    description: "MPEG-1 Audio Frame Header (32-bit: sync, version, layer, bitrate, sampling rate)",
    pattern: "sync:11u,version:2u,layer:2u,crc:1u,bitrate:4u,samplerate:2u,padding:1u,private:1u,channel:2u,mode_ext:2u,copyright:1u,original:1u,emphasis:2u",
    unit: "32",
    sample: "sample.mp3",
    count: "5",
    sink: "json"
  },
  {
    id: "mpeg-ts",
    name: "MPEG-TS PID",
    icon: "📺",
    category: "multimedia",
    description: "188-byte packet stream, extracting 13-bit Packet Identifier (PID) at offset 11",
    rawUnit: "1504",
    offset: "11",
    unit: "13",
    pattern: "pid:13u",
    sample: "sample.ts",
    count: "10",
    sink: "json"
  },
  {
    id: "nvfp4",
    name: "Blackwell NVFP4",
    icon: "⚡",
    category: "ai",
    description: "NVIDIA Blackwell sub-byte 4-bit float pairs (E2M1) packed two per byte",
    pattern: "w0:4E,w1:4E",
    unit: "8",
    sample: "sample_nvfp4.bin",
    count: "8",
    sink: "json"
  },
  {
    id: "fp6-e3m2",
    name: "OCP FP6",
    icon: "🔬",
    category: "ai",
    description: "OCP Microscaling 6-bit unaligned floats (4 weights across 24 bits / 3 bytes)",
    pattern: "w0:6E,w1:6E,w2:6E,w3:6E",
    unit: "24",
    sample: "sample_fp6.bin",
    count: "4",
    sink: "json"
  },
  {
    id: "wav-header",
    name: "WAV PCM",
    icon: "🔊",
    category: "multimedia",
    description: "RIFF 44-byte standard audio header with sample rate, channels, bit depth",
    pattern: "riff:32C,file_size:32U,wave:32C,fmt:32C,fmt_size:32U,audio_fmt:16U,channels:16U,sample_rate:32U,byte_rate:32U,block_align:16U,bits_per_sample:16U,data:32C,data_size:32U",
    unit: "352",
    sample: "sample.wav",
    count: "1",
    sink: "json",
    littleEndian: true
  },
  {
    id: "jpeg-sof0",
    name: "JPEG SOF0",
    icon: "🖼️",
    category: "multimedia",
    description: "JPEG Frame Header SOF0 (image width, height, chroma subsampling)",
    skipBits: "16",
    pattern: "precision:8u,height:16u,width:16u,components:8u",
    unit: "48",
    sample: "sample.jpg",
    count: "1",
    sink: "json"
  },
  {
    id: "ipv4-header",
    name: "IPv4 Packet",
    icon: "🌐",
    category: "network",
    description: "IPv4 20-byte base packet header (version, IHL, TTL, protocol, IPs)",
    pattern: "version:4U,ihl:4U,dscp:6U,ecn:2U,total_len:16U,id:16U,flags:3U,frag_offset:13U,ttl:8U,proto:8U,checksum:16U,src_ip:32U,dst_ip:32U",
    unit: "160",
    sample: "sample_ipv4.bin",
    count: "1",
    sink: "json"
  },
  {
    id: "safetensors-fp8",
    name: "Safetensors FP8",
    icon: "🧠",
    category: "ai",
    description: "Skip 240-byte JSON header and slice FP8 E4M3 neural weights",
    skipBits: "240B",
    pattern: "8E",
    unit: "8",
    sample: "sample.safetensors",
    count: "8",
    sink: "json"
  }
];

// App State
const state = {
  activeTab: "file",
  file: null,
  fileBase64: null,
  sampleName: "sample.mp3",
  activePresetId: "mp3-header",
  lastResult: null,
  lastMimeType: "text/plain"
};

// DOM Elements
const el = {
  presetChips: document.getElementById("preset-chips"),
  sourceTabs: document.getElementById("source-tabs"),
  dropzone: document.getElementById("file-dropzone"),
  fileInput: document.getElementById("file-input"),
  fileInfo: document.getElementById("file-info"),
  infoFileName: document.getElementById("info-file-name"),
  infoFileSize: document.getElementById("info-file-size"),
  sampleSelect: document.getElementById("sample-select"),
  btnLoadSample: document.getElementById("btn-load-sample"),
  btnProbeFile: document.getElementById("btn-probe-file"),
  btnProbeAction: document.getElementById("btn-probe-action"),
  inputTuplesText: document.getElementById("input-tuples-text"),
  
  // Slicing inputs
  inputPattern: document.getElementById("input-pattern"),
  patternMeta: document.getElementById("pattern-meta"),
  patternDiagram: document.getElementById("pattern-diagram"),
  inputUnit: document.getElementById("input-unit"),
  outputUnit: document.getElementById("output-unit"),
  outputPattern: document.getElementById("output-pattern"),
  
  // Advanced container controls
  inputRawUnit: document.getElementById("input-raw-unit"),
  inputOffset: document.getElementById("input-offset"),
  inputSkipBits: document.getElementById("input-skip-bits"),
  inputSkipUnits: document.getElementById("input-skip-units"),
  inputGap: document.getElementById("input-gap"),
  countLimit: document.getElementById("count-limit"),
  
  // Checkboxes
  dropPartialEof: document.getElementById("drop-partial-eof"),
  littleEndian: document.getElementById("little-endian"),
  reverseBytes: document.getElementById("reverse-bytes"),
  reverseUnit: document.getElementById("reverse-unit"),
  noSeek: document.getElementById("no-seek"),
  assertAligned: document.getElementById("assert-aligned"),
  
  // Pipelines
  pipelineRound: document.getElementById("pipeline-round"),
  pipelineRearrange: document.getElementById("pipeline-rearrange"),
  pipelineFilter: document.getElementById("pipeline-filter"),
  pipelineMath: document.getElementById("pipeline-math"),
  
  // Output Sinks
  sinkRadios: document.getElementsByName("output-sink"),
  
  // Actions & Previews
  btnRunSlice: document.getElementById("btn-run-slice"),
  btnCopyCmd: document.getElementById("btn-copy-cmd"),
  copiedPill: document.getElementById("copied-pill"),
  cliPreview: document.getElementById("cli-command-preview"),
  
  // Results
  resultsBadge: document.getElementById("results-badge"),
  btnDownloadResult: document.getElementById("btn-download-result"),
  emptyState: document.getElementById("empty-state"),
  spinnerOverlay: document.getElementById("spinner-overlay"),
  outputContainer: document.getElementById("output-container"),
  outputText: document.getElementById("output-text")
};

// Initialize Application
function init() {
  renderPresets();
  setupEventListeners();
  loadPreset(PRESETS[0]);
  updateCliPreview();
  checkServerStatus();
}

// Check Backend Engine Status
async function checkServerStatus() {
  const statusIndicator = document.getElementById("server-status");
  try {
    const res = await fetch("/api/status");
    if (res.ok) {
      statusIndicator.innerHTML = '<span class="status-dot online"></span><span class="status-label">Engine Ready</span>';
    } else {
      statusIndicator.innerHTML = '<span class="status-dot online"></span><span class="status-label">Engine Active</span>';
    }
  } catch (_) {
    statusIndicator.innerHTML = '<span class="status-dot online"></span><span class="status-label">Local Engine</span>';
  }
}

// Render Presets Bar
function renderPresets() {
  el.presetChips.innerHTML = PRESETS.map(p => `
    <button type="button" class="preset-chip ${p.id === state.activePresetId ? 'active' : ''}" data-id="${p.id}" title="${p.description}">
      <span>${p.icon}</span>
      <span>${p.name}</span>
    </button>
  `).join("");
}

// Apply Selected Preset
function loadPreset(preset) {
  state.activePresetId = preset.id;
  renderPresets();
  
  // Form fields
  el.inputPattern.value = preset.pattern || "";
  el.inputUnit.value = preset.pattern ? "" : (preset.unit || "");
  if (preset.pattern && preset.unit) {
    el.inputUnit.placeholder = `${preset.unit} (from pattern)`;
  } else {
    el.inputUnit.placeholder = "8";
  }
  el.outputUnit.value = "";
  el.outputPattern.value = "";
  el.inputRawUnit.value = preset.rawUnit || "";
  el.inputOffset.value = preset.offset || "";
  el.inputSkipBits.value = preset.skipBits || "";
  el.inputSkipUnits.value = "";
  el.inputGap.value = preset.gap || "";
  el.countLimit.value = preset.count || "10";
  
  el.littleEndian.checked = !!preset.littleEndian;
  el.dropPartialEof.checked = true;
  el.reverseBytes.checked = false;
  el.reverseUnit.checked = false;
  el.noSeek.checked = false;
  el.assertAligned.checked = false;
  
  el.pipelineRound.value = "";
  el.pipelineRearrange.value = "";
  el.pipelineFilter.value = "";
  el.pipelineMath.value = "";
  
  // Set sample
  if (preset.sample) {
    el.sampleSelect.value = preset.sample;
    state.sampleName = preset.sample;
  }
  
  // Set output sink
  const targetSink = preset.sink || "json";
  for (const radio of el.sinkRadios) {
    radio.checked = (radio.value === targetSink);
    radio.closest(".sink-radio-card").classList.toggle("active", radio.checked);
  }
  
  updatePatternVisualization();
  updateCliPreview();
}

// Parse Pattern String into Fields
function parsePatternString(patStr) {
  if (!patStr || !patStr.trim()) return [];
  const items = [];
  const parts = patStr.split(",");
  
  for (const part of parts) {
    const trimmed = part.trim();
    if (!trimmed) continue;
    
    let name = null;
    let typeSpec = trimmed;
    
    if (trimmed.includes(":")) {
      const idx = trimmed.indexOf(":");
      name = trimmed.substring(0, idx).trim();
      typeSpec = trimmed.substring(idx + 1).trim();
    }
    
    // Parse length & type
    const match = typeSpec.match(/^(\d*)([a-zA-Z])$/);
    if (match) {
      const countStr = match[1];
      const typeChar = match[2];
      let bits = countStr ? parseInt(countStr, 10) : defaultTypeBits(typeChar);
      items.push({
        name: name || `f${items.length}`,
        type: typeChar,
        bits: bits,
        raw: trimmed
      });
    } else {
      items.push({
        name: name || `f${items.length}`,
        type: "?",
        bits: 8,
        raw: trimmed
      });
    }
  }
  return items;
}

function defaultTypeBits(ch) {
  switch (ch.toUpperCase()) {
    case 'U': case 'X': return 1;
    case 'B': case 'C': return 8;
    case 'E': case 'Q': return 8;
    case 'H': case 'Y': return 16;
    case 'F': return 32;
    case 'D': return 64;
    default: return 8;
  }
}

function getTypeClass(typeChar) {
  switch (typeChar.toUpperCase()) {
    case 'U': return 'type-uint';
    case 'S': case 'M': return 'type-int';
    case 'F': case 'D': case 'E': case 'Q': case 'H': case 'Y': return 'type-float';
    case 'C': case 'B': return 'type-char';
    case 'X': return 'type-discard';
    case 'K': return 'type-counter';
    default: return 'type-uint';
  }
}

// Update Diagram Visualization
function updatePatternVisualization() {
  const pat = el.inputPattern.value.trim();
  const fields = parsePatternString(pat);
  
  if (fields.length === 0) {
    el.patternMeta.textContent = "Total bits: 0";
    el.patternDiagram.innerHTML = '<div class="diagram-empty-hint">Enter a pattern above or select a preset to visualize bit allocation</div>';
    return;
  }
  
  let totalBits = 0;
  let html = "";
  
  for (const f of fields) {
    const startBit = totalBits;
    totalBits += f.bits;
    const endBit = totalBits - 1;
    const rangeStr = (f.bits === 1) ? `bit ${startBit}` : `[${startBit}..${endBit}]`;
    const typeClass = getTypeClass(f.type);
    
    html += `
      <div class="pattern-field-block ${typeClass}" title="${f.name}: ${f.bits} bits (${f.type})">
        <span class="field-name">${f.name}</span>
        <span class="field-bits">${f.bits}b (${rangeStr})</span>
      </div>
    `;
  }
  
  el.patternMeta.textContent = `Total: ${totalBits} bits (${(totalBits / 8).toFixed(1)} bytes)`;
  el.patternDiagram.innerHTML = html;
  
  // Auto update input unit if empty
  if (!el.inputUnit.value && totalBits > 0) {
    el.inputUnit.placeholder = `${totalBits} (from pattern)`;
  }
}

// Generate the Exact CLI Command
function buildCliArgs() {
  const args = [];
  
  // Source
  if (state.activeTab === "file" && state.file) {
    args.push(`--input-file=${state.file.name}`);
  } else if (state.activeTab === "samples") {
    args.push(`--input-file=contrib/data/${el.sampleSelect.value}`);
  } else if (state.activeTab === "generator") {
    const gen = document.querySelector('input[name="generator-type"]:checked')?.value || "counter";
    args.push(`--input-${gen}`);
  } else if (state.activeTab === "tuples") {
    args.push(`--input-tuples`);
  } else {
    // Default fallback
    args.push(`--input-file=contrib/data/${state.sampleName}`);
  }
  
  // Pattern / Preset / Unit
  const pat = el.inputPattern.value.trim();
  if (pat) {
    args.push(`--input-pattern=${pat}`);
  }
  
  const inUnit = el.inputUnit.value.trim();
  if (inUnit && !pat) {
    args.push(`--input-unit=${inUnit}`);
  }
  
  const outPat = el.outputPattern.value.trim();
  if (outPat) {
    args.push(`--output-pattern=${outPat}`);
  }

  const outUnit = el.outputUnit.value.trim();
  if (outUnit && !outPat) {
    args.push(`--output-unit=${outUnit}`);
  }
  
  // Container & seeking
  const rawUnit = el.inputRawUnit.value.trim();
  if (rawUnit) args.push(`--input-raw-unit=${rawUnit}`);
  
  const offset = el.inputOffset.value.trim();
  if (offset && offset !== "0") args.push(`--input-offset=${offset}`);
  
  const skipBits = el.inputSkipBits.value.trim();
  if (skipBits) args.push(`--input-skip-bits=${skipBits}`);
  
  const skipUnits = el.inputSkipUnits.value.trim();
  if (skipUnits) args.push(`--input-skip-units=${skipUnits}`);
  
  const gap = el.inputGap.value.trim();
  if (gap) args.push(`--input-gap=${gap}`);
  
  const count = el.countLimit.value.trim();
  if (count) args.push(`--count=${count}`);
  
  // Checkboxes
  if (el.dropPartialEof.checked) args.push("--drop-partial-eof");
  if (el.littleEndian.checked) args.push("--input-little-endian");
  if (el.reverseBytes.checked) args.push("--input-reverse-bytes");
  if (el.reverseUnit.checked) args.push("--input-reverse-unit");
  if (el.noSeek.checked) args.push("--no-seek");
  if (el.assertAligned.checked) args.push("--input-assert-aligned");
  
  // Pipeline
  const round = el.pipelineRound.value.trim();
  if (round) args.push(`--round=${round}`);
  
  const rearrange = el.pipelineRearrange.value.trim();
  if (rearrange) args.push(`--rearrange=${rearrange}`);
  
  const filter = el.pipelineFilter.value.trim();
  if (filter) args.push(`--filter=${filter}`);
  
  const math = el.pipelineMath.value.trim();
  if (math) {
    if (math.includes("=")) {
      const [op, val] = math.split("=");
      args.push(`--${op}=${val}`);
    } else {
      args.push(`--${math}`);
    }
  }
  
  // Output sink
  const sink = document.querySelector('input[name="output-sink"]:checked')?.value || "json";
  switch (sink) {
    case "json": args.push("--output-json"); break;
    case "csv": args.push("--output-csv"); break;
    case "visual": args.push("--output-visual"); break;
    case "hex": args.push("--output-hex"); break;
    case "bits": args.push("--output-bits"); break;
    case "binary": break; // default binary
  }
  
  return args;
}

function updateCliPreview() {
  const args = buildCliArgs();
  el.cliPreview.textContent = `bdd ${args.join(" ")}`;
}

// Event Listeners
function setupEventListeners() {
  // Preset clicks
  el.presetChips.addEventListener("click", (e) => {
    const chip = e.target.closest(".preset-chip");
    if (!chip) return;
    const presetId = chip.getAttribute("data-id");
    const p = PRESETS.find(x => x.id === presetId);
    if (p) loadPreset(p);
  });
  
  // Source tabs
  el.sourceTabs.addEventListener("click", (e) => {
    const btn = e.target.closest(".tab-btn");
    if (!btn) return;
    const tabName = btn.getAttribute("data-tab");
    
    document.querySelectorAll(".tab-btn").forEach(b => b.classList.remove("active"));
    document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));
    
    btn.classList.add("active");
    document.getElementById(`tab-${tabName}`).classList.add("active");
    state.activeTab = tabName;
    updateCliPreview();
  });
  
  // Unit Quick Pills
  document.querySelectorAll(".unit-quick-pills .pill").forEach(pill => {
    pill.addEventListener("click", () => {
      el.inputUnit.value = pill.getAttribute("data-unit");
      updateCliPreview();
    });
  });
  
  // Drag & drop
  el.dropzone.addEventListener("click", () => el.fileInput.click());
  
  el.fileInput.addEventListener("change", (e) => {
    if (e.target.files && e.target.files[0]) {
      handleFileUpload(e.target.files[0]);
    }
  });
  
  ["dragenter", "dragover"].forEach(evt => {
    el.dropzone.addEventListener(evt, (e) => {
      e.preventDefault();
      e.stopPropagation();
      el.dropzone.classList.add("dragover");
    });
  });
  
  ["dragleave", "drop"].forEach(evt => {
    el.dropzone.addEventListener(evt, (e) => {
      e.preventDefault();
      e.stopPropagation();
      el.dropzone.classList.remove("dragover");
    });
  });
  
  el.dropzone.addEventListener("drop", (e) => {
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      handleFileUpload(e.dataTransfer.files[0]);
    }
  });
  
  // Sample select
  el.btnLoadSample.addEventListener("click", () => {
    state.sampleName = el.sampleSelect.value;
    updateCliPreview();
    triggerNotification(`Fixture "${state.sampleName}" loaded into memory`);
  });
  
  // Pattern input changes
  el.inputPattern.addEventListener("input", () => {
    updatePatternVisualization();
    updateCliPreview();
  });
  
  // General form changes
  const watchedInputs = [
    el.inputUnit, el.outputUnit, el.outputPattern,
    el.inputRawUnit, el.inputOffset, el.inputSkipBits, el.inputSkipUnits,
    el.inputGap, el.countLimit, el.dropPartialEof, el.littleEndian,
    el.reverseBytes, el.reverseUnit, el.noSeek, el.assertAligned,
    el.pipelineRound, el.pipelineRearrange, el.pipelineFilter, el.pipelineMath,
    el.sampleSelect, el.inputTuplesText
  ];
  
  watchedInputs.forEach(input => {
    input.addEventListener("input", updateCliPreview);
    input.addEventListener("change", updateCliPreview);
  });
  
  // Radio cards
  document.querySelectorAll('input[name="output-sink"]').forEach(radio => {
    radio.addEventListener("change", () => {
      document.querySelectorAll('.sink-radio-card').forEach(c => c.classList.remove("active"));
      radio.closest('.sink-radio-card').classList.add("active");
      updateCliPreview();
    });
  });
  
  // Action Buttons
  el.btnRunSlice.addEventListener("click", executeSlice);
  el.btnProbeFile.addEventListener("click", executeProbe);
  el.btnProbeAction.addEventListener("click", executeProbe);
  
  // Copy CLI command
  el.btnCopyCmd.addEventListener("click", () => {
    const cmd = el.cliPreview.textContent;
    navigator.clipboard.writeText(cmd).then(() => {
      el.copiedPill.classList.remove("hidden");
      setTimeout(() => el.copiedPill.classList.add("hidden"), 2000);
    });
  });
  
  // Download result button
  el.btnDownloadResult.addEventListener("click", downloadResult);
  
  // Keyboard shortcut Ctrl+Enter
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      executeSlice();
    }
  });
}

// Handle File Drop / Select
function handleFileUpload(file) {
  state.file = file;
  el.infoFileName.textContent = file.name;
  el.infoFileSize.textContent = formatBytes(file.size);
  el.fileInfo.classList.remove("hidden");
  
  const reader = new FileReader();
  reader.onload = (e) => {
    const arrayBuffer = e.target.result;
    const bytes = new Uint8Array(arrayBuffer);
    let binary = "";
    const len = bytes.byteLength;
    for (let i = 0; i < len; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    state.fileBase64 = btoa(binary);
    updateCliPreview();
  };
  reader.readAsArrayBuffer(file);
}

function formatBytes(bytes) {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function triggerNotification(msg) {
  el.resultsBadge.textContent = msg;
  el.resultsBadge.className = "badge success";
}

// Execute Slicing via Backend API
async function executeSlice() {
  el.emptyState.classList.add("hidden");
  el.spinnerOverlay.classList.remove("hidden");
  el.outputContainer.classList.add("hidden");
  el.btnRunSlice.disabled = true;
  el.resultsBadge.textContent = "Processing...";
  el.resultsBadge.className = "badge";
  
  const args = buildCliArgs();
  const sink = document.querySelector('input[name="output-sink"]:checked')?.value || "json";
  
  const payload = {
    args: args,
    sink: sink,
    file_name: state.file ? state.file.name : (state.sampleName || null),
    file_base64: state.file ? state.fileBase64 : null,
    tuples_text: (state.activeTab === "tuples") ? el.inputTuplesText.value : null
  };
  
  try {
    const response = await fetch("/api/process", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    
    const data = await response.json();
    
    el.spinnerOverlay.classList.add("hidden");
    el.outputContainer.classList.remove("hidden");
    el.btnRunSlice.disabled = false;
    
    if (data.success) {
      el.resultsBadge.textContent = "Success";
      el.resultsBadge.className = "badge success";
      el.btnDownloadResult.disabled = false;
      
      state.lastResult = data.stdout || (data.binary_base64 ? atob(data.binary_base64) : "");
      state.lastMimeType = (sink === "json") ? "application/json" : (sink === "csv") ? "text/csv" : "text/plain";
      
      // Render text output
      if (sink === "json" && data.stdout) {
        try {
          // Prettify JSON if lines of JSON
          const lines = data.stdout.trim().split("\n");
          if (lines.length === 1) {
            el.outputText.textContent = JSON.stringify(JSON.parse(lines[0]), null, 2);
          } else {
            el.outputText.textContent = lines.map(l => {
              try { return JSON.stringify(JSON.parse(l)); } catch (_) { return l; }
            }).join("\n");
          }
        } catch (_) {
          el.outputText.textContent = data.stdout;
        }
      } else {
        el.outputText.textContent = data.stdout || (data.binary_base64 ? `[Binary Stream: ${Math.round(data.binary_base64.length * 0.75)} bytes generated]` : "No output produced.");
      }
    } else {
      el.resultsBadge.textContent = "Error";
      el.resultsBadge.className = "badge error";
      el.outputText.textContent = data.stderr || "Unknown execution failure occurred.";
      el.btnDownloadResult.disabled = true;
    }
  } catch (err) {
    el.spinnerOverlay.classList.add("hidden");
    el.outputContainer.classList.remove("hidden");
    el.btnRunSlice.disabled = false;
    el.resultsBadge.textContent = "Connection Error";
    el.resultsBadge.className = "badge error";
    el.outputText.textContent = `Failed to connect to bdd API server:\n${err.message}\n\nPlease ensure 'bdd --serve' is running.`;
    el.btnDownloadResult.disabled = true;
  }
}

// Execute Probe
async function executeProbe() {
  el.emptyState.classList.add("hidden");
  el.spinnerOverlay.classList.remove("hidden");
  el.outputContainer.classList.add("hidden");
  el.resultsBadge.textContent = "Probing...";
  
  const payload = {
    target: state.file ? state.file.name : (state.sampleName || "sample.mp3"),
    file_base64: state.file ? state.fileBase64 : null
  };
  
  try {
    const res = await fetch("/api/probe", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    const data = await res.json();
    
    el.spinnerOverlay.classList.add("hidden");
    el.outputContainer.classList.remove("hidden");
    
    if (data.report) {
      el.resultsBadge.textContent = "Probe Complete";
      el.resultsBadge.className = "badge success";
      el.outputText.textContent = JSON.stringify(data.report, null, 2);
    } else {
      el.outputText.textContent = data.error || "Unable to probe target.";
    }
  } catch (err) {
    el.spinnerOverlay.classList.add("hidden");
    el.outputContainer.classList.remove("hidden");
    el.outputText.textContent = `Probe error: ${err.message}`;
  }
}

// Download Sliced Results
function downloadResult() {
  if (!state.lastResult) return;
  const sink = document.querySelector('input[name="output-sink"]:checked')?.value || "txt";
  const ext = (sink === "json") ? "json" : (sink === "csv") ? "csv" : (sink === "binary") ? "bin" : "txt";
  const filename = `bdd_export_${Date.now()}.${ext}`;
  
  const blob = new Blob([state.lastResult], { type: state.lastMimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// Start app
document.addEventListener("DOMContentLoaded", init);
