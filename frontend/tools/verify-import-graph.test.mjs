import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  activeSourcePathDisposition,
  analyzeImportGraph,
  repositoryStaticContractFixture,
  validateActiveFileSystemEntry,
  validateDenyDocument,
  validatePackageScripts,
  validateRepositoryStaticContracts,
  validateRepositoryStaticFileUniverse,
  validateSourceTextEncoding,
} from './verify-import-graph.mjs';

const toolsDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(toolsDirectory, '../..');
const sharedDenyPath = resolve(repositoryRoot, 'tools/architecture/public-source-deny.json');

function denyInput() {
  return structuredClone(JSON.parse(readFileSync(sharedDenyPath, 'utf8')).capabilities);
}

function denyDocument() {
  return structuredClone(JSON.parse(readFileSync(sharedDenyPath, 'utf8')));
}

function baselineFiles() {
  return new Map([
    [
      'src/entry.local.tsx',
      "import { LocalCompositionRoot } from './ui/local/LocalCompositionRoot';\nvoid LocalCompositionRoot;\n",
    ],
    [
      'src/entry.public.tsx',
      "import { PublicCompositionRoot } from './ui/public/PublicCompositionRoot';\nvoid PublicCompositionRoot;\n",
    ],
    [
      'src/ui/shared/SharedApplication.tsx',
      'export function SharedApplication() { return null; }\n',
    ],
    [
      'src/ui/local/LocalCompositionRoot.tsx',
      "import { SharedApplication } from '../shared/SharedApplication';\nexport const LocalCompositionRoot = SharedApplication;\n",
    ],
    [
      'src/ui/public/PublicCompositionRoot.tsx',
      "import { SharedApplication } from '../shared/SharedApplication';\nexport const PublicCompositionRoot = SharedApplication;\n",
    ],
  ]);
}

function analyze(files = baselineFiles(), publicSourceDeny = denyInput()) {
  return analyzeImportGraph({
    allowedExternalPackages: new Set(['react', 'react-dom']),
    files,
    publicSourceDeny,
  });
}

test('accepts the explicit dual-entry shared graph and 18/215 deny policy', () => {
  const report = analyze();
  assert.equal(report.state, 'PASS', report.errors.join('\n'));
  assert.equal(report.activeUniverse.fileCount, 5);
  assert.equal(report.publicSourceDeny.checkedCount, 18);
  assert.equal(report.publicSourceDeny.checkedMarkerCount, 215);
  assert.deepEqual(report.closures.local, [
    'src/entry.local.tsx',
    'src/ui/local/LocalCompositionRoot.tsx',
    'src/ui/shared/SharedApplication.tsx',
  ]);
  assert.deepEqual(report.closures.public, [
    'src/entry.public.tsx',
    'src/ui/public/PublicCompositionRoot.tsx',
    'src/ui/shared/SharedApplication.tsx',
  ]);
});

test('rejects the desired-watch authority from shared and public source', () => {
  // The authority is a local-only capability: its rows live in the local
  // database, its route is a local route, and the public build has no
  // durable per-user state to hold intent in. These three markers are what
  // keeps that true once it stops being obvious -- the authority itself, the
  // generation barrier it is versioned by, and the epoch its materialization
  // is keyed on. Shared is inside the public closure, so a helper parked
  // there leaks just as surely as one written in a public file.
  for (const [file, source, marker] of [
    [
      'src/ui/shared/SharedApplication.tsx',
      'export const desiredWatchPath = "/api/v1/local/desired-watch";\n',
      'desired_watch',
    ],
    [
      'src/ui/public/PublicCompositionRoot.tsx',
      "import { SharedApplication } from '../shared/SharedApplication';\n"
        + 'export const PublicCompositionRoot = SharedApplication;\n'
        + 'export const authorityGeneration = 1;\n',
      'authority_generation',
    ],
    [
      'src/ui/shared/SharedApplication.tsx',
      'export function SharedApplication() { return null; }\n'
        + 'export const materializationEpoch = 1;\n',
      'materialization_epoch',
    ],
  ]) {
    const files = baselineFiles();
    files.set(file, source);
    const report = analyze(files);
    assert.equal(report.state, 'FAIL', `${marker} must be refused`);
    assert.ok(
      report.errors.some((error) => error.includes(marker)),
      `${marker}: ${report.errors.join('\n')}`,
    );
  }
});

test('accepts only the self-contained deny policy identity', () => {
  assert.deepEqual(validateDenyDocument(denyDocument()), []);
  const liveDependency = denyDocument();
  liveDependency.runtimeSourceRequired = true;
  assert.match(validateDenyDocument(liveDependency).join('\n'), /must not require/u);
});

test('rejects shared-to-local, public-to-local, and local-to-public edges', async (t) => {
  const cases = [
    {
      file: 'src/ui/shared/SharedApplication.tsx',
      source:
        "import { LocalCompositionRoot } from '../local/LocalCompositionRoot';\nexport const SharedApplication = LocalCompositionRoot;\n",
      expected: /forbidden shared -> local edge/u,
    },
    {
      file: 'src/ui/public/PublicCompositionRoot.tsx',
      source:
        "import { LocalCompositionRoot } from '../local/LocalCompositionRoot';\nexport const PublicCompositionRoot = LocalCompositionRoot;\n",
      expected: /forbidden public -> local edge/u,
    },
    {
      file: 'src/ui/local/LocalCompositionRoot.tsx',
      source:
        "import { PublicCompositionRoot } from '../public/PublicCompositionRoot';\nexport const LocalCompositionRoot = PublicCompositionRoot;\n",
      expected: /forbidden local -> public edge/u,
    },
  ];
  for (const fixture of cases) {
    await t.test(fixture.expected.source, () => {
      const files = baselineFiles();
      files.set(fixture.file, fixture.source);
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), fixture.expected);
    });
  }
});

test('rejects imports into the frozen legacy source graph', () => {
  const files = baselineFiles();
  files.set('src/entry.local.tsx', "import { App } from './App';\nvoid App;\n");
  const report = analyze(files);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /inactive\/legacy source/u);
});

test('rejects non-literal, multi-argument, and concatenated dynamic imports', async (t) => {
  for (const expression of [
    'import(moduleName)',
    "import('../local/LocalCompositionRoot', {})",
    "import('../local/' + moduleName)",
  ]) {
    await t.test(expression, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/shared/SharedApplication.tsx',
        `const moduleName = 'LocalCompositionRoot';\nvoid ${expression};\nexport function SharedApplication() { return null; }\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), /dynamic import must contain exactly one string literal/u);
    });
  }
});

test('rejects import.meta.glob discovery', () => {
  const files = baselineFiles();
  files.set(
    'src/ui/shared/SharedApplication.tsx',
    "const modules = import.meta.glob('../**/*.tsx');\nvoid modules;\nexport function SharedApplication() { return null; }\n",
  );
  const report = analyze(files);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /import\.meta\.glob is forbidden/u);
});

test('rejects manual React JSX runtime imports that can synthesize script or iframe tags', () => {
  const files = baselineFiles();
  files.set(
    'src/ui/public/PublicCompositionRoot.tsx',
    "import { jsx } from 'react/jsx-runtime';\nconst value = jsx('script', { src: '/x' });\nvoid value;\nexport function PublicCompositionRoot() { return null; }\n",
  );
  const report = analyze(files);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /manual JSX runtime imports are forbidden/u);
});

test('rejects direct and statically disguised P4 SOURCE-denied markers', async (t) => {
  const cases = [
    ['identifier', 'const saved_view = true;'],
    ['no-substitution template', 'const label = `saved_view`;'],
    ['interpolated static template', "const label = `saved_${'view'}`;"],
    ['static string concatenation', "const label = 'saved_' + ('view');"],
    [
      'cross-binding static concatenation',
      "const prefix = 'saved_';\nconst suffix = 'view';\nconst label = prefix + suffix;",
    ],
    ['cross-binding interpolated template', "const suffix = '_view';\nconst label = `saved${suffix}`;"],
    ['typed interpolated template', "const suffix: string = '_view';\nconst label = `saved${suffix}`;"],
    ['const-asserted interpolated template', "const suffix = '_view' as const;\nconst label = `saved${suffix}`;"],
    ['parenthesized const assertion', "const suffix = ('_view' as const);\nconst label = `saved${suffix}`;"],
    ['non-null asserted binding', "const suffix = '_view'!;\nconst label = `saved${suffix}`;"],
    ['escaped interpolated template', "const suffix = '_view';\nconst label = `\\u0073aved${suffix}`;"],
    ['mutable split marker', "let first = 'saved', second = '_view';\nconst label = first + second;"],
  ];
  for (const [name, declaration] of cases) {
    await t.test(name, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/public/PublicCompositionRoot.tsx',
        `import { SharedApplication } from '../shared/SharedApplication';\n${declaration}\nvoid label;\nexport const PublicCompositionRoot = SharedApplication;\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), /P4-D-SAVED_VIEWS-SOURCE/u);
    });
  }
});

test('rescans interpolated templates before continuing import analysis', () => {
  const files = baselineFiles();
  files.set(
    'src/ui/public/PublicCompositionRoot.tsx',
    "import { SharedApplication } from '../shared/SharedApplication';\nconst suffix = 'x';\nconst value = `a${suffix}`;\nimport { LocalCompositionRoot } from '../local/LocalCompositionRoot';\nvoid value;\nvoid LocalCompositionRoot;\nexport const PublicCompositionRoot = SharedApplication;\n",
  );
  const report = analyze(files);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /forbidden public -> local edge/u);
});

test('ignores deny-marker examples that exist only in comments', () => {
  const files = baselineFiles();
  files.set(
    'src/ui/public/PublicCompositionRoot.tsx',
    "import { SharedApplication } from '../shared/SharedApplication';\n// `saved_view` is intentionally unsupported.\nexport const PublicCompositionRoot = SharedApplication;\n",
  );
  const report = analyze(files);
  assert.equal(report.state, 'PASS', report.errors.join('\n'));
});

test('rejects missing, substituted, and reordered deny-policy markers', async (t) => {
  const cases = [
    ['missing row', (rows) => rows.slice(0, -1)],
    [
      'substituted marker',
      (rows) => {
        rows[0].markers[0] = 'substituted_marker';
        return rows;
      },
    ],
    [
      'reordered marker',
      (rows) => {
        [rows[0].markers[0], rows[0].markers[1]] = [rows[0].markers[1], rows[0].markers[0]];
        return rows;
      },
    ],
    [
      'substituted validation identity',
      (rows) => {
        rows[0].validationId = 'P4-Z-CHANGED-SOURCE';
        return rows;
      },
    ],
  ];
  for (const [name, mutate] of cases) {
    await t.test(name, () => {
      const report = analyze(baselineFiles(), mutate(denyInput()));
      assert.equal(report.state, 'FAIL');
      assert.match(
        report.errors.join('\n'),
        /exactly 18 capabilities|repository policy/u,
      );
    });
  }
});

test('auditor evidence: Worker and eval source/code generation fail closed', async (t) => {
  const cases = [
    ['Worker', "const workerInstance = new Worker('/worker.js');\nvoid workerInstance;"],
    ['eval', "const result = eval('1 + 1');\nvoid result;"],
  ];
  for (const [construct, source] of cases) {
    await t.test(construct, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/shared/SharedApplication.tsx',
        `${source}\nexport function SharedApplication() { return null; }\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), new RegExp(`prohibited.*${construct}`, 'u'));
      assert.ok(report.prohibitedConstructs.count > 0);
    });
  }
});

test('rejects the remaining common source-loading/code-generation constructs', async (t) => {
  const cases = [
    ['SharedWorker', "const value = new SharedWorker('/worker.js');"],
    [
      'Worker URL',
      "const value = new Worker(new URL('../local/LocalCompositionRoot.tsx', import.meta.url));",
    ],
    ['importScripts', "const value = importScripts('/module.js');"],
    ['serviceWorker', 'const value = navigator.serviceWorker;'],
    ['Function', "const value = Function('return 1');"],
    ["createElement('script')", "const value = document.createElement('script');"],
    ['createElement template', 'const value = document.createElement(`script`);'],
    [
      'computed createElement',
      "const value = document['create' + 'Element']('script');",
    ],
    ['JSX script', 'const value = <script src="/module.js" />;'],
    ['JSX iframe', 'const value = <iframe src="/module.js" />;'],
    [
      'static timer string binding',
      "const prefix = 'ale'; const suffix = 'rt(1)'; const code = prefix + suffix; const value = setTimeout(code);",
    ],
  ];
  for (const [construct, source] of cases) {
    await t.test(construct, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/shared/SharedApplication.tsx',
        `${source}\nvoid value;\nexport function SharedApplication() { return null; }\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), /prohibited source-loading\/code-generation construct/u);
    });
  }
});

test('rejects statically computed source-loading and code-generation sinks', async (t) => {
  const cases = [
    [
      'computed createElement and script tag',
      "const methodTail = 'Element';\nconst tagTail = 'ipt';\ndocument[`create${methodTail}`](`scr${tagTail}`);",
    ],
    ['direct iframe creation', "document.createElement('iframe');"],
    [
      'cross-binding local HTML escape',
      "const first = '/loc';\nconst second = 'al.html';\nwindow.open(first + second);",
    ],
    ['array-backed timer code', "const code = ['alert(1)'];\nsetTimeout(code[0]);"],
    [
      'computed Worker constructor',
      "const first = 'Wor', second = 'ker';\nconst Constructor = globalThis[first + second];\nnew Constructor('/x');",
    ],
    [
      'computed eval property',
      "const first = 'ev', second = 'al';\nglobalThis[first + second]('1+1');",
    ],
    [
      'typed and const-asserted computed eval property',
      "const first: string = 'ev', second = 'al' as const;\nglobalThis[first + second]('1+1');",
    ],
    [
      'generic typed and asserted computed eval property',
      "const first: Record<string, string> = 'ev' as any, second: string = 'al';\nglobalThis[(first as any) + (second as any)]('1+1');",
    ],
    [
      'destructured computed eval property',
      "const [first, second] = ['ev', 'al'] as const;\nglobalThis[first + second]('1+1');",
    ],
    [
      'comparison before split computed eval property',
      "const noop = 1 < 2, first = 'ev', second = 'al';\nvoid noop;\nglobalThis[first + second]('1+1');",
    ],
    ['dynamic JSX script tag', "const Tag = 'script';\nconst value = <Tag src='/x' />;\nvoid value;"],
    ['dynamic JSX iframe tag', "const Tag = 'iframe';\nconst value = <Tag src='/x' />;\nvoid value;"],
    [
      'createElementNS script',
      "document.createElementNS('http://www.w3.org/1999/xhtml', 'script');",
    ],
    ['document write', "document.write('<script src=/x></script>');"],
    [
      'contextual fragment',
      "const range = document.createRange();\nrange.createContextualFragment('<script src=/x></script>');",
    ],
  ];
  for (const [name, source] of cases) {
    await t.test(name, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/public/PublicCompositionRoot.tsx',
        `import { SharedApplication } from '../shared/SharedApplication';\n${source}\nexport const PublicCompositionRoot = SharedApplication;\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL', `${name}: ${report.errors.join('\n')}`);
      assert.ok(report.prohibitedConstructs.count > 0);
    });
  }
});

test('allows URL construction for the permitted direct Section link', () => {
  const files = baselineFiles();
  files.set(
    'src/ui/public/PublicCompositionRoot.tsx',
    "import { SharedApplication } from '../shared/SharedApplication';\nconst directSectionUrl = new URL('/section/12345', window.location.origin);\nvoid directSectionUrl;\nexport const PublicCompositionRoot = SharedApplication;\n",
  );
  const report = analyze(files);
  assert.equal(report.state, 'PASS', report.errors.join('\n'));
});

test('rejects public references to the local HTML or source roots', async (t) => {
  for (const source of [
    "const value = window.open('/local.html');",
    "const value = '/src/ui/' + 'local/LocalCompositionRoot.tsx';",
  ]) {
    await t.test(source, () => {
      const files = baselineFiles();
      files.set(
        'src/ui/public/PublicCompositionRoot.tsx',
        `import { SharedApplication } from '../shared/SharedApplication';\n${source}\nvoid value;\nexport const PublicCompositionRoot = SharedApplication;\n`,
      );
      const report = analyze(files);
      assert.equal(report.state, 'FAIL');
      assert.match(report.errors.join('\n'), /public target escape/u);
    });
  }
});

test('active filesystem policy rejects alternate source extensions and symlink entries', () => {
  assert.equal(activeSourcePathDisposition('src/ui/shared/Allowed.tsx'), 'source');
  assert.throws(
    () => activeSourcePathDisposition('src/ui/shared/Bypass.js'),
    /source-like extension \.js is forbidden/u,
  );
  assert.throws(
    () =>
      validateActiveFileSystemEntry(
        'src/entry.public.tsx',
        {
          isDirectory: () => false,
          isFile: () => false,
          isSymbolicLink: () => true,
        },
        'file',
      ),
    /symbolic links are forbidden/u,
  );
  assert.throws(
    () =>
      validateActiveFileSystemEntry(
        'src/ui/public',
        {
          isDirectory: () => false,
          isFile: () => false,
          isSymbolicLink: () => true,
        },
        'directory',
      ),
    /symbolic links are forbidden/u,
  );
});

test('HTML stays single-entry while Vite and package checks allow unrelated evolution', () => {
  const valid = repositoryStaticContractFixture();
  assert.deepEqual(validateRepositoryStaticContracts(valid), []);

  const extraRoot = repositoryStaticContractFixture();
  extraRoot.set(
    'public.html',
    extraRoot
      .get('public.html')
      .replace('</body>', '<script type="module" src="/src/extra.tsx"></script>\n  </body>'),
  );
  assert.match(validateRepositoryStaticContracts(extraRoot).join('\n'), /exactly one script/u);

  const extraHtmlFile = repositoryStaticContractFixture();
  extraHtmlFile.set('extra.html', '<script type="module" src="/src/extra.tsx"></script>\n');
  assert.match(
    validateRepositoryStaticContracts(extraHtmlFile).join('\n'),
    /HTML\/Vite file universe mismatch/u,
  );

  const alias = repositoryStaticContractFixture();
  alias.set(
    'vite.config.ts',
    alias.get('vite.config.ts').replace("appType: 'spa',", "appType: 'spa',\n    resolve: { alias: {} },"),
  );
  assert.deepEqual(validateRepositoryStaticContracts(alias), []);

  const unsafeAssets = repositoryStaticContractFixture();
  unsafeAssets.set(
    'vite.config.ts',
    unsafeAssets.get('vite.config.ts').replace('publicDir: false', 'publicDir: true'),
  );
  assert.match(validateRepositoryStaticContracts(unsafeAssets).join('\n'), /disable recursive public assets/u);

  const crossedTarget = repositoryStaticContractFixture();
  crossedTarget.set(
    'vite.public.config.ts',
    crossedTarget.get('vite.public.config.ts').replace("createTargetConfig('public')", "createTargetConfig('local')"),
  );
  assert.match(validateRepositoryStaticContracts(crossedTarget).join('\n'), /only the public target/u);

  for (const filePath of [
    'vite.extra.config.mjs',
    'vite.extra.config.js',
    'vite.extra.config.mts',
    'Vite.extra.config.ts',
    'sub/vite.config.ts',
    'sub/index.html',
  ]) {
    assert.match(
      validateRepositoryStaticFileUniverse([
        'local.html',
        'public.html',
        'vite.config.ts',
        'vite.local.config.ts',
        'vite.public.config.ts',
        filePath,
      ]).join('\n'),
      /HTML\/Vite file universe mismatch/u,
    );
  }

  const packageJson = JSON.parse(readFileSync(resolve(repositoryRoot, 'frontend/package.json'), 'utf8'));
  assert.deepEqual(validatePackageScripts(packageJson.scripts), []);
  assert.equal(packageJson.scripts['test:i18n'], 'vitest run --config vitest.config.ts');
  assert.match(packageJson.scripts.verify, /npm run test:i18n/u);
  const extraScripts = { ...packageJson.scripts, 'build:alternate': 'vite --config sub/vite.config.ts' };
  assert.deepEqual(validatePackageScripts(extraScripts), []);
  const missingArtifactCheck = {
    ...packageJson.scripts,
    'build:public': 'vite build --config vite.public.config.ts',
  };
  assert.match(validatePackageScripts(missingArtifactCheck).join('\n'), /verify-target-build\.mjs public/u);
  const lateSourceGuard = {
    ...packageJson.scripts,
    'build:local': 'vite build --config vite.local.config.ts && npm run guard:imports && node ./tools/verify-target-build.mjs local',
  };
  assert.match(validatePackageScripts(lateSourceGuard).join('\n'), /npm run guard:imports then vite build/u);
});

test('rejects a raw NUL byte in an active source file', () => {
  // A NUL makes Git call the file binary, which takes it out of diffs,
  // out of grep, and out of every security inventory that walks text
  // sources -- while it keeps compiling and shipping. That happened once
  // here, so the guard is a build failure rather than a convention.
  const nul = String.fromCharCode(0);
  const files = baselineFiles();
  files.set(
    'src/ui/shared/watch/intent.ts',
    `export const key = 'term${nul}campus';\n`,
  );
  const report = analyzeImportGraph({
    files,
    publicSourceDeny: denyInput(),
    allowedExternalPackages: ['react'],
  });
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /must not contain a raw NUL byte/u);

  // The escape is the same string at runtime and stays readable to every
  // tool, so it must pass.
  const escaped = baselineFiles();
  escaped.set(
    'src/ui/shared/watch/intent.ts',
    "export const key = `term\\u0000campus`;\n",
  );
  assert.deepEqual(validateSourceTextEncoding(escaped), []);
});

test('the shipped active sources contain no raw NUL bytes', () => {
  const guardPath = resolve(toolsDirectory, 'verify-import-graph.mjs');
  const result = spawnSync(process.execPath, [guardPath, '--json'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stdout);
  assert.doesNotMatch(result.stdout, /raw NUL byte/u);
});

test('--json emits JSON on stdout only', () => {
  const guardPath = resolve(toolsDirectory, 'verify-import-graph.mjs');
  const result = spawnSync(process.execPath, [guardPath, '--json'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
  });
  assert.equal(result.stderr, '');
  assert.equal(result.status, 0, result.stdout);
  assert.equal(JSON.parse(result.stdout).state, 'PASS');
});

test('unknown CLI arguments fail closed with JSON-only stdout', () => {
  const guardPath = resolve(toolsDirectory, 'verify-import-graph.mjs');
  const result = spawnSync(process.execPath, [guardPath, '--json', '--jsno'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
  });
  assert.equal(result.stderr, '');
  assert.equal(result.status, 1);
  const report = JSON.parse(result.stdout);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /unknown argument/u);
});
