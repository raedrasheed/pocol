/* Classic worker: build copies this file and the pinned solc/soljson.js together.
 * Sources never leave the browser. All imported .sol files must be in the project.
 */
/* global importScripts */
let compiler;
let compilerVersion;
let queue = Promise.resolve();

async function loadCompiler() {
  if (compiler) return;
  importScripts('./soljson.js');
  const module = self.Module;
  if (!module) throw new Error('The bundled Solidity compiler is missing.');
  if (!module._solidity_compile) {
    await new Promise((resolve, reject) => {
      module.onRuntimeInitialized = resolve;
      module.onAbort = () => reject(new Error('The Solidity compiler could not initialize.'));
    });
  }
  compilerVersion = module.cwrap('solidity_version', 'string', [])();
  if (!compilerVersion.startsWith('0.8.38+')) throw new Error('Compiler version mismatch. This IDE requires its bundled Solidity 0.8.38 compiler.');
  compiler = module.cwrap('solidity_compile', 'string', ['string', 'number', 'number']);
}

function position(content, byteOffset) {
  const prefix = new TextDecoder().decode(new TextEncoder().encode(content).slice(0, Math.max(0, byteOffset)));
  const lines = prefix.split('\n');
  return { line: lines.length, column: lines[lines.length - 1].length + 1 };
}

function normalizeOutput(output, input, durationMs) {
  const diagnostics = (output.errors || []).map(error => {
    const loc = error.sourceLocation;
    const source = loc && input.sources[loc.file]?.content;
    const start = typeof source === 'string' ? position(source, loc.start) : null;
    const end = typeof source === 'string' ? position(source, loc.end) : null;
    return {
      severity: error.severity || 'error', message: error.message || 'Compiler error',
      formattedMessage: error.formattedMessage || error.message, errorCode: error.errorCode,
      file: loc?.file || null, start: loc?.start, end: loc?.end,
      line: start?.line, column: start?.column, endLine: end?.line, endColumn: end?.column,
    };
  });
  const contracts = [];
  for (const [file, units] of Object.entries(output.contracts || {})) {
    for (const [name, unit] of Object.entries(units)) {
      const bytecode = unit.evm?.bytecode?.object || '';
      const deployed = unit.evm?.deployedBytecode?.object || '';
      const unlinkedLibraries = Object.entries(unit.evm?.bytecode?.linkReferences || {}).flatMap(([source, libraries]) => Object.keys(libraries).map(library => `${source}:${library}`));
      contracts.push({
        id: `${file}:${name}`, name, contractName: name, file,
        abi: unit.abi || [], bytecode: `0x${bytecode}`, deployedBytecode: `0x${deployed}`,
        bytecodeBytes: Math.floor(bytecode.length / 2), runtimeBytes: Math.floor(deployed.length / 2),
        deployable: bytecode.length > 0 && /^[a-fA-F0-9]+$/.test(bytecode) && unlinkedLibraries.length === 0,
        unlinkedLibraries, methodIdentifiers: unit.evm?.methodIdentifiers || {},
        gasEstimates: unit.evm?.gasEstimates || {}, storageLayout: unit.storageLayout,
        metadata: unit.metadata, compilerVersion, evmVersion: 'cancun',
        sourceMap: unit.evm?.bytecode?.sourceMap || '', deployedSourceMap: unit.evm?.deployedBytecode?.sourceMap || '',
      });
    }
  }
  return {
    success: !diagnostics.some(error => error.severity === 'error'),
    compiler: compilerVersion, evmVersion: 'cancun', settings: input.settings,
    contracts, diagnostics, durationMs,
  };
}

self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try {
      if (!data?.id || data.input?.language !== 'Solidity' || data.input?.settings?.evmVersion !== 'cancun') throw new Error('Invalid compilation request.');
      await loadCompiler();
      const started = performance.now();
      // No import callback: solc resolves only the files explicitly supplied by the user.
      const output = JSON.parse(compiler(JSON.stringify(data.input), 0, 0));
      const result = normalizeOutput(output, data.input, Math.round(performance.now() - started));
      self.postMessage({ id: data.id, result });
      if (self.Module._solidity_reset) self.Module.cwrap('solidity_reset', null, [])();
    } catch (error) {
      self.postMessage({ id: data?.id, error: String(error?.message || error) });
    }
  });
};
