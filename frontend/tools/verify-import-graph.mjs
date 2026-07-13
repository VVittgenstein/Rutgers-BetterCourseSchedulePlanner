import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, posix, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  createScanner,
  LanguageVariant,
  SyntaxKind,
} from 'typescript/unstable/ast';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const frontendRoot = resolve(scriptDirectory, '..');

const ENTRY_PATHS = Object.freeze({
  local: 'src/entry.local.tsx',
  public: 'src/entry.public.tsx',
});

const REQUIRED_COMPOSITION_PATHS = Object.freeze({
  local: ['src/ui/local/LocalCompositionRoot.tsx', 'src/ui/shared/SharedApplication.tsx'],
  public: ['src/ui/public/PublicCompositionRoot.tsx', 'src/ui/shared/SharedApplication.tsx'],
});

const ACTIVE_DIRECTORIES = Object.freeze(['src/ui/shared', 'src/ui/local', 'src/ui/public']);
const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx']);
const SOURCE_LIKE_EXTENSIONS = new Set([
  '.astro',
  '.cjs',
  '.css',
  '.cts',
  '.html',
  '.js',
  '.json',
  '.jsx',
  '.less',
  '.mjs',
  '.mts',
  '.sass',
  '.scss',
  '.svelte',
  '.styl',
  '.stylus',
  '.svg',
  '.vue',
  '.wasm',
]);

export const EXPECTED_PUBLIC_SOURCE_DENIES = Object.freeze([
  ['P4-D-SAVED_VIEWS-SOURCE', 'SAVED_VIEWS'],
  ['P4-D-PERSISTENT_PREFS-SOURCE', 'PERSISTENT_PREFS'],
  ['P4-D-PERSISTENT_HISTORY-SOURCE', 'PERSISTENT_HISTORY'],
  ['P4-D-PERSISTENT_SELECTION-SOURCE', 'PERSISTENT_SELECTION'],
  ['P4-D-PERSISTENT_ACTIVE_WATCH-SOURCE', 'PERSISTENT_ACTIVE_WATCH'],
  ['P4-D-EMAIL-SOURCE', 'EMAIL'],
  ['P4-D-DISCORD-SOURCE', 'DISCORD'],
  ['P4-D-SHARE_LINKS-SOURCE', 'SHARE_LINKS'],
  ['P4-D-WAITLIST-SOURCE', 'WAITLIST'],
  ['P4-D-COMPACT-SOURCE', 'COMPACT'],
  ['P4-D-MACOS-SOURCE', 'MACOS'],
  ['P4-D-NATIVE_APP-SOURCE', 'NATIVE_APP'],
  ['P4-D-SYSTEM_NOTIFICATIONS-SOURCE', 'SYSTEM_NOTIFICATIONS'],
  ['P4-D-WEB_PUSH-SOURCE', 'WEB_PUSH'],
  ['P4-D-CALENDAR-SOURCE', 'CALENDAR'],
  ['P4-D-QUIET_SNOOZE-SOURCE', 'QUIET_SNOOZE'],
  ['P4-D-PERSISTENT_SUBSCRIPTION_IDENTITY-SOURCE', 'PERSISTENT_SUBSCRIPTION_IDENTITY'],
  ['P4-D-LOCAL_RESET-SOURCE', 'LOCAL_RESET'],
]);

const EXPECTED_PUBLIC_SOURCE_MARKER_COUNT = 212;
const EXPECTED_PUBLIC_SOURCE_ROWS_SHA256 =
  '521CFF222462CC06D015A2954B5D317B5B1CBE7047E27945E0374F3122E74593';

const ALLOWED_EDGE_TARGETS = Object.freeze({
  'local-entry': new Set(['local', 'shared']),
  local: new Set(['local', 'shared']),
  'public-entry': new Set(['public', 'shared']),
  public: new Set(['public', 'shared']),
  shared: new Set(['shared']),
});

const STATIC_REPOSITORY_CONTRACT = Object.freeze({
  'build/target-build.ts': `export const frontendTargets = {
  local: {
    devPort: 5174,
    html: 'local.html',
    outDir: 'dist/local',
  },
  public: {
    devPort: 5175,
    html: 'public.html',
    outDir: 'dist/public',
  },
} as const;

export type FrontendTarget = keyof typeof frontendTargets;

export function getTargetBuild(target: FrontendTarget) {
  return frontendTargets[target];
}
`,
  'local.html': `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>RBCSP Local</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/entry.local.tsx"></script>
  </body>
</html>
`,
  'public.html': `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>RBCSP Public</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/entry.public.tsx"></script>
  </body>
</html>
`,
  'vite.config.ts': `import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

import react from '@vitejs/plugin-react';
import type { UserConfig } from 'vite';

import { getTargetBuild, type FrontendTarget } from './build/target-build';

const frontendRoot = fileURLToPath(new URL('.', import.meta.url));

export function createTargetConfig(target: FrontendTarget): UserConfig {
  const descriptor = getTargetBuild(target);

  return {
    appType: 'spa',
    base: './',
    plugins: [react()],
    publicDir: false,
    root: frontendRoot,
    server: {
      port: descriptor.devPort,
      strictPort: true,
    },
    build: {
      emptyOutDir: true,
      manifest: 'asset-manifest.json',
      outDir: resolve(frontendRoot, descriptor.outDir),
      rollupOptions: {
        input: resolve(frontendRoot, descriptor.html),
      },
      sourcemap: false,
    },
  };
}
`,
  'vite.local.config.ts': `import { createTargetConfig } from './vite.config';

export default createTargetConfig('local');
`,
  'vite.public.config.ts': `import { createTargetConfig } from './vite.config';

export default createTargetConfig('public');
`,
});

const EXPECTED_ROOT_STATIC_FILES = Object.freeze([
  'local.html',
  'public.html',
  'vite.config.ts',
  'vite.local.config.ts',
  'vite.public.config.ts',
]);

const EXPECTED_PACKAGE_SCRIPTS = Object.freeze({
  guard: 'npm run guard:imports && npm run test:guard',
  'guard:imports': 'node ./tools/verify-import-graph.mjs',
  'test:guard': 'node --test ./tools/verify-import-graph.test.mjs',
  typecheck: 'tsc --build tsconfig.json --pretty false',
  'typecheck:shared': 'tsc --build tsconfig.shared.json --pretty false',
  'typecheck:local': 'tsc --build tsconfig.local.json --pretty false',
  'typecheck:public': 'tsc --build tsconfig.public.json --pretty false',
  'typecheck:build': 'tsc --build tsconfig.node.json --pretty false',
  build: 'npm run build:local && npm run build:public',
  'build:local': 'vite build --config vite.local.config.ts',
  'build:public': 'vite build --config vite.public.config.ts',
  'dev:local': 'vite --config vite.local.config.ts --open /local.html',
  'dev:public': 'vite --config vite.public.config.ts --open /public.html',
  verify: 'npm run guard && npm run typecheck && npm run build',
  'preview:local': 'vite preview --config vite.local.config.ts',
  'preview:public': 'vite preview --config vite.public.config.ts',
});

function toPosixPath(value) {
  return posix.normalize(value.replaceAll('\\', '/')).replace(/^\.\//u, '');
}

function normalizeText(value) {
  return value.replaceAll('\r\n', '\n').replaceAll('\r', '\n').trimEnd() + '\n';
}

function normalizeAuditToken(value) {
  return value.toLocaleLowerCase('en-US').replace(/[^a-z0-9]/gu, '');
}

function classifyActivePath(filePath) {
  if (filePath === ENTRY_PATHS.local) return 'local-entry';
  if (filePath === ENTRY_PATHS.public) return 'public-entry';
  if (filePath.startsWith('src/ui/shared/')) return 'shared';
  if (filePath.startsWith('src/ui/local/')) return 'local';
  if (filePath.startsWith('src/ui/public/')) return 'public';
  return 'inactive';
}

function externalPackageName(specifier) {
  const parts = specifier.split('/');
  return specifier.startsWith('@') ? parts.slice(0, 2).join('/') : parts[0];
}

function resolveInternalImport(importer, specifier, files) {
  const base = toPosixPath(posix.join(posix.dirname(importer), specifier));
  const candidates = extname(base)
    ? [base]
    : [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`];

  return candidates.find((candidate) => files.has(candidate));
}

function scanTokens(sourceText) {
  const scanner = createScanner(true, LanguageVariant.JSX, sourceText);
  const tokens = [];
  const templateExpressionBraceDepth = [];
  for (let kind = scanner.scan(); kind !== SyntaxKind.EndOfFile; kind = scanner.scan()) {
    if (
      kind === SyntaxKind.CloseBraceToken &&
      templateExpressionBraceDepth.length > 0 &&
      templateExpressionBraceDepth.at(-1) === 0
    ) {
      kind = scanner.reScanTemplateToken(false);
      if (kind === SyntaxKind.TemplateTail) templateExpressionBraceDepth.pop();
    } else if (kind === SyntaxKind.TemplateHead) {
      templateExpressionBraceDepth.push(0);
    } else if (templateExpressionBraceDepth.length > 0 && kind === SyntaxKind.OpenBraceToken) {
      templateExpressionBraceDepth[templateExpressionBraceDepth.length - 1] += 1;
    } else if (templateExpressionBraceDepth.length > 0 && kind === SyntaxKind.CloseBraceToken) {
      templateExpressionBraceDepth[templateExpressionBraceDepth.length - 1] -= 1;
    }
    tokens.push({
      end: scanner.getTokenEnd(),
      kind,
      start: scanner.getTokenStart(),
      text: scanner.getTokenText(),
      value: scanner.getTokenValue(),
    });
  }
  return tokens;
}

function isStringLikeToken(token) {
  return [
    SyntaxKind.StringLiteral,
    SyntaxKind.NoSubstitutionTemplateLiteral,
    SyntaxKind.TemplateHead,
    SyntaxKind.TemplateMiddle,
    SyntaxKind.TemplateTail,
  ].includes(token?.kind);
}

function evaluateStaticStringExpression(tokens, bindings, arrayBindings = new Map()) {
  tokens = stripStaticExpressionTypeSyntax(tokens);
  let combined = '';
  let expectValue = true;
  let valueCount = 0;
  let parenthesisDepth = 0;
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === SyntaxKind.OpenParenToken) {
      parenthesisDepth += 1;
      continue;
    }
    if (token.kind === SyntaxKind.CloseParenToken) {
      parenthesisDepth -= 1;
      if (parenthesisDepth < 0) return null;
      continue;
    }
    if (expectValue) {
      if (isStringLikeToken(token)) {
        combined += token.value;
      } else if (token.kind === SyntaxKind.Identifier && bindings.has(token.value)) {
        combined += bindings.get(token.value);
      } else if (
        token.kind === SyntaxKind.Identifier &&
        arrayBindings.has(token.value) &&
        tokens[index + 1]?.kind === SyntaxKind.OpenBracketToken &&
        /^\d+$/u.test(tokens[index + 2]?.text ?? '') &&
        tokens[index + 3]?.kind === SyntaxKind.CloseBracketToken
      ) {
        const value = arrayBindings.get(token.value)[Number(tokens[index + 2].text)];
        if (typeof value !== 'string') return null;
        combined += value;
        index += 3;
      } else {
        return null;
      }
      valueCount += 1;
      expectValue = false;
      continue;
    }
    if (token.kind !== SyntaxKind.PlusToken) return null;
    expectValue = true;
  }
  return valueCount > 0 && !expectValue && parenthesisDepth === 0 ? combined : null;
}

function evaluateStaticStringArray(tokens, bindings, arrayBindings) {
  if (
    tokens[0]?.kind !== SyntaxKind.OpenBracketToken ||
    tokens.at(-1)?.kind !== SyntaxKind.CloseBracketToken
  ) {
    return null;
  }
  const values = [];
  let start = 1;
  let parenthesisDepth = 0;
  for (let index = 1; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === SyntaxKind.OpenParenToken) parenthesisDepth += 1;
    if (token.kind === SyntaxKind.CloseParenToken) parenthesisDepth -= 1;
    const atSeparator = parenthesisDepth === 0 && (
      token.kind === SyntaxKind.CommaToken || token.kind === SyntaxKind.CloseBracketToken
    );
    if (!atSeparator) continue;
    const value = evaluateStaticStringExpression(tokens.slice(start, index), bindings, arrayBindings);
    if (value === null) return null;
    values.push(value);
    start = index + 1;
  }
  return values;
}

function splitConstDeclarations(tokens) {
  const declarations = [];
  let start = 0;
  let bracketDepth = 0;
  let braceDepth = 0;
  let angleDepth = 0;
  let parenthesisDepth = 0;
  let declarationHasEquals = false;
  for (let index = 0; index <= tokens.length; index += 1) {
    const token = tokens[index];
    if (token?.kind === SyntaxKind.OpenBracketToken) bracketDepth += 1;
    if (token?.kind === SyntaxKind.CloseBracketToken) bracketDepth -= 1;
    if (token?.kind === SyntaxKind.OpenBraceToken) braceDepth += 1;
    if (token?.kind === SyntaxKind.CloseBraceToken) braceDepth -= 1;
    if (!declarationHasEquals && token?.kind === SyntaxKind.LessThanToken) angleDepth += 1;
    if (!declarationHasEquals && token?.kind === SyntaxKind.GreaterThanToken) angleDepth -= 1;
    if (token?.kind === SyntaxKind.OpenParenToken) parenthesisDepth += 1;
    if (token?.kind === SyntaxKind.CloseParenToken) parenthesisDepth -= 1;
    if (token?.kind === SyntaxKind.EqualsToken && bracketDepth === 0 && braceDepth === 0) {
      declarationHasEquals = true;
    }
    if (index === tokens.length || (
      token.kind === SyntaxKind.CommaToken &&
      bracketDepth === 0 &&
      braceDepth === 0 &&
      angleDepth === 0 &&
      parenthesisDepth === 0
    )) {
      declarations.push(tokens.slice(start, index));
      start = index + 1;
      declarationHasEquals = false;
    }
  }
  return declarations;
}

function stripStaticExpressionTypeSyntax(tokens) {
  const result = [];
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === SyntaxKind.AsKeyword || token.kind === SyntaxKind.SatisfiesKeyword) {
      index += 1;
      while (
        index < tokens.length &&
        ![
          SyntaxKind.CloseParenToken,
          SyntaxKind.CommaToken,
          SyntaxKind.PlusToken,
        ].includes(tokens[index].kind)
      ) {
        index += 1;
      }
      index -= 1;
      continue;
    }
    if (token.kind === SyntaxKind.ExclamationToken) continue;
    result.push(token);
  }
  return result;
}

function collectStaticStringBindings(tokens) {
  const bindings = new Map();
  const arrayBindings = new Map();
  for (let index = 0; index < tokens.length; index += 1) {
    if (![SyntaxKind.ConstKeyword, SyntaxKind.LetKeyword, SyntaxKind.VarKeyword].includes(tokens[index].kind)) continue;
    let end = index + 1;
    while (end < tokens.length && tokens[end].kind !== SyntaxKind.SemicolonToken) end += 1;
    for (const declaration of splitConstDeclarations(tokens.slice(index + 1, end))) {
      const equalsIndex = declaration.findIndex((token) => token.kind === SyntaxKind.EqualsToken);
      if (equalsIndex < 1) continue;
      const expression = stripStaticExpressionTypeSyntax(declaration.slice(equalsIndex + 1));
      const arrayValue = evaluateStaticStringArray(expression, bindings, arrayBindings);
      if (declaration[0]?.kind === SyntaxKind.OpenBracketToken) {
        const closePattern = declaration.findIndex(
          (token, patternIndex) =>
            patternIndex < equalsIndex && token.kind === SyntaxKind.CloseBracketToken,
        );
        const pattern = declaration.slice(1, closePattern);
        const names = pattern.filter((token) => token.kind === SyntaxKind.Identifier);
        const patternIsSimple = pattern.every(
          (token) =>
            token.kind === SyntaxKind.Identifier || token.kind === SyntaxKind.CommaToken,
        );
        if (arrayValue !== null && patternIsSimple && names.length === arrayValue.length) {
          names.forEach((name, valueIndex) => bindings.set(name.value, arrayValue[valueIndex]));
        }
        continue;
      }
      if (declaration[0]?.kind !== SyntaxKind.Identifier) continue;
      if (arrayValue !== null) {
        arrayBindings.set(declaration[0].value, arrayValue);
        continue;
      }
      const value = evaluateStaticStringExpression(expression, bindings, arrayBindings);
      if (value !== null) bindings.set(declaration[0].value, value);
    }
    index = end;
  }
  return { arrayBindings, bindings };
}

function decodeStaticTemplateChunk(chunk) {
  let valid = true;
  let value = chunk.replace(/\\\r?\n[\t ]*/gu, '');
  value = value.replace(/\\u\{([0-9a-fA-F]{1,6})\}/gu, (_match, hex) => {
    const codePoint = Number.parseInt(hex, 16);
    if (codePoint > 0x10ffff) {
      valid = false;
      return '';
    }
    return String.fromCodePoint(codePoint);
  });
  value = value.replace(/\\u([0-9a-fA-F]{4})/gu, (_match, hex) =>
    String.fromCodePoint(Number.parseInt(hex, 16)));
  value = value.replace(/\\x([0-9a-fA-F]{2})/gu, (_match, hex) =>
    String.fromCodePoint(Number.parseInt(hex, 16)));
  const simpleEscapes = new Map([
    ['0', '\0'],
    ['b', '\b'],
    ['f', '\f'],
    ['n', '\n'],
    ['r', '\r'],
    ['t', '\t'],
    ['v', '\v'],
    ['`', '`'],
    ['\\', '\u0000'],
  ]);
  value = value.replace(/\\([0bfnrtv`\\])/gu, (_match, escape) => simpleEscapes.get(escape));
  if (value.includes('\\')) valid = false;
  return valid ? value.replaceAll('\u0000', '\\') : null;
}

function evaluateStaticTemplate(templateText, bindings, arrayBindings) {
  if (!templateText.startsWith('`') || !templateText.endsWith('`')) return null;
  const body = templateText.slice(1, -1);
  let value = '';
  let cursor = 0;
  for (const match of body.matchAll(/\$\{([^{}]*)\}/gsu)) {
    const literal = decodeStaticTemplateChunk(body.slice(cursor, match.index));
    if (literal === null) return null;
    const expressionValue = evaluateStaticStringExpression(
      scanTokens(match[1]),
      bindings,
      arrayBindings,
    );
    if (expressionValue === null) return null;
    value += literal + expressionValue;
    cursor = match.index + match[0].length;
  }
  const tail = decodeStaticTemplateChunk(body.slice(cursor));
  if (tail === null || tail.includes('${')) return null;
  return value + tail;
}

function collectAuditTokens(tokens, sourceText) {
  const auditTokens = new Set();
  const { arrayBindings: staticStringArrayBindings, bindings: staticStringBindings } =
    collectStaticStringBindings(tokens);
  const add = (value) => {
    const normalized = normalizeAuditToken(value);
    if (normalized.length > 0) auditTokens.add(normalized);
  };

  for (const token of tokens) {
    if (
      token.kind === SyntaxKind.Identifier ||
      token.kind === SyntaxKind.JsxText ||
      token.kind === SyntaxKind.RegularExpressionLiteral ||
      isStringLikeToken(token)
    ) {
      add(token.value);
    }
  }
  for (const value of staticStringBindings.values()) add(value);
  for (const values of staticStringArrayBindings.values()) values.forEach(add);

  for (let start = 0; start < tokens.length; start += 1) {
    for (let end = start + 1; end <= Math.min(tokens.length, start + 24); end += 1) {
      const value = evaluateStaticStringExpression(
        tokens.slice(start, end),
        staticStringBindings,
        staticStringArrayBindings,
      );
      if (value !== null) add(value);
    }
  }

  // Fold the common static concatenation forms: 'saved_' + 'views' and
  // 'saved_' + ('views'). Scanner trivia is already omitted.
  for (let start = 0; start < tokens.length; start += 1) {
    if (!isStringLikeToken(tokens[start])) continue;
    let cursor = start;
    let combined = tokens[start].value;
    let literalCount = 1;
    let sawPlus = false;
    while (cursor + 1 < tokens.length) {
      cursor += 1;
      const candidate = tokens[cursor];
      if (candidate.kind === SyntaxKind.PlusToken) {
        sawPlus = true;
        continue;
      }
      if (
        candidate.kind === SyntaxKind.OpenParenToken ||
        candidate.kind === SyntaxKind.CloseParenToken
      ) {
        continue;
      }
      if (sawPlus && isStringLikeToken(candidate)) {
        combined += candidate.value;
        literalCount += 1;
        sawPlus = false;
        continue;
      }
      break;
    }
    if (literalCount > 1) add(combined);
  }

  // Scanner template rescanning is intentionally not used here. Auditing the
  // raw template body catches no-substitution templates and static interpolated
  // forms such as `saved_${'views'}` without executing the expression.
  for (const template of sourceText.matchAll(/`(?:\\.|[^`])*`/gsu)) {
    const startsAtTemplateToken = tokens.some(
      (token) => token.start === template.index && isStringLikeToken(token),
    );
    if (startsAtTemplateToken) {
      add(template[0]);
      const staticValue = evaluateStaticTemplate(
        template[0],
        staticStringBindings,
        staticStringArrayBindings,
      );
      if (staticValue !== null) add(staticValue);
    }
  }

  return { auditTokens, staticStringArrayBindings, staticStringBindings };
}

function firstCallArgument(tokens, openParenIndex) {
  let parenthesisDepth = 0;
  let bracketDepth = 0;
  for (let index = openParenIndex + 1; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === SyntaxKind.OpenParenToken) parenthesisDepth += 1;
    if (token.kind === SyntaxKind.OpenBracketToken) bracketDepth += 1;
    if (token.kind === SyntaxKind.CloseBracketToken) bracketDepth -= 1;
    if (token.kind === SyntaxKind.CloseParenToken) {
      if (parenthesisDepth === 0 && bracketDepth === 0) {
        return tokens.slice(openParenIndex + 1, index);
      }
      parenthesisDepth -= 1;
    }
    if (
      token.kind === SyntaxKind.CommaToken &&
      parenthesisDepth === 0 &&
      bracketDepth === 0
    ) {
      return tokens.slice(openParenIndex + 1, index);
    }
  }
  return [];
}

function collectProhibitedConstructs(
  filePath,
  tokens,
  auditTokens,
  staticStringBindings,
  staticStringArrayBindings,
) {
  const occurrences = [];
  const add = (construct, index) => {
    const key = `${construct}|${index}`;
    if (occurrences.some((item) => item.key === key)) return;
    occurrences.push({ construct, file: filePath, index, key });
  };

  const globallyForbiddenNames = new Map([
    ['eval', 'eval'],
    ['Function', 'Function'],
    ['importScripts', 'importScripts'],
    ['serviceWorker', 'serviceWorker'],
    ['Worker', 'Worker'],
    ['SharedWorker', 'SharedWorker'],
    ['dangerouslySetInnerHTML', 'dangerouslySetInnerHTML'],
    ['innerHTML', 'innerHTML'],
    ['outerHTML', 'outerHTML'],
    ['insertAdjacentHTML', 'insertAdjacentHTML'],
    ['DOMParser', 'DOMParser'],
    ['createContextualFragment', 'createContextualFragment'],
  ]);

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (
      token.kind === SyntaxKind.Identifier ||
      token.kind === SyntaxKind.StringLiteral ||
      token.kind === SyntaxKind.NoSubstitutionTemplateLiteral
    ) {
      const forbidden = globallyForbiddenNames.get(token.value);
      if (forbidden) add(forbidden, index);
    }

    if (
      token.kind === SyntaxKind.Identifier &&
      token.value === 'createElement' &&
      tokens[index + 1]?.kind === SyntaxKind.OpenParenToken &&
      ['script', 'iframe'].includes(
        evaluateStaticStringExpression(
          firstCallArgument(tokens, index + 1),
          staticStringBindings,
          staticStringArrayBindings,
        )?.toLocaleLowerCase('en-US'),
      )
    ) {
      const tagName = evaluateStaticStringExpression(
        firstCallArgument(tokens, index + 1),
        staticStringBindings,
        staticStringArrayBindings,
      ).toLocaleLowerCase('en-US');
      add(`createElement('${tagName}')`, index);
    }

    if (
      token.kind === SyntaxKind.Identifier &&
      (token.value === 'setTimeout' || token.value === 'setInterval') &&
      tokens[index + 1]?.kind === SyntaxKind.OpenParenToken &&
      evaluateStaticStringExpression(
        firstCallArgument(tokens, index + 1),
        staticStringBindings,
        staticStringArrayBindings,
      ) !== null
    ) {
      add(`${token.value}(string)`, index);
    }

    if (
      token.kind === SyntaxKind.LessThanToken &&
      tokens[index + 1]?.kind === SyntaxKind.Identifier &&
      ['script', 'iframe'].includes(tokens[index + 1]?.value.toLocaleLowerCase('en-US'))
    ) {
      add(`<${tokens[index + 1].value.toLocaleLowerCase('en-US')}>`, index);
    }

    if (
      token.kind === SyntaxKind.LessThanToken &&
      tokens[index + 1]?.kind === SyntaxKind.Identifier &&
      staticStringBindings.has(tokens[index + 1].value) &&
      ['script', 'iframe'].includes(
        staticStringBindings.get(tokens[index + 1].value).toLocaleLowerCase('en-US'),
      )
    ) {
      add(`dynamic JSX <${staticStringBindings.get(tokens[index + 1].value)}>`, index);
    }
  }

  if (auditTokens.has('createelement')) {
    for (const tagName of ['script', 'iframe']) {
      if (auditTokens.has(tagName)) {
        add(`computed createElement('${tagName}')`, `audit:createelement-${tagName}`);
      }
    }
  }
  if (auditTokens.has('createelementns')) {
    for (const tagName of ['script', 'iframe']) {
      if (auditTokens.has(tagName)) {
        add(`computed createElementNS('${tagName}')`, `audit:createelementns-${tagName}`);
      }
    }
  }
  if (auditTokens.has('document')) {
    for (const method of ['write', 'writeln']) {
      if (auditTokens.has(method)) add(`document.${method}`, `audit:document-${method}`);
    }
  }

  for (const value of auditTokens) {
    for (const [normalized, construct] of [
      ['eval', 'computed eval'],
      ['function', 'computed Function'],
      ['importscripts', 'computed importScripts'],
      ['serviceworker', 'computed serviceWorker'],
      ['worker', 'computed Worker'],
      ['sharedworker', 'computed SharedWorker'],
    ]) {
      if (value === normalized) add(construct, `audit:${normalized}`);
    }
  }

  return occurrences.map(({ key: _key, ...occurrence }) => occurrence);
}

function literalOnlyCallArgument(tokens, openParenIndex) {
  return (
    tokens[openParenIndex]?.kind === SyntaxKind.OpenParenToken &&
    tokens[openParenIndex + 1]?.kind === SyntaxKind.StringLiteral &&
    tokens[openParenIndex + 2]?.kind === SyntaxKind.CloseParenToken
  );
}

function collectSourceFacts(filePath, sourceText) {
  const imports = [];
  const errors = [];
  let importMetaGlobCount = 0;
  let nonLiteralDynamicImportCount = 0;
  const tokens = scanTokens(sourceText);
  const { auditTokens, staticStringArrayBindings, staticStringBindings } =
    collectAuditTokens(tokens, sourceText);
  const prohibitedConstructs = collectProhibitedConstructs(
    filePath,
    tokens,
    auditTokens,
    staticStringBindings,
    staticStringArrayBindings,
  );

  for (const occurrence of prohibitedConstructs) {
    errors.push(`${filePath}: prohibited source-loading/code-generation construct ${occurrence.construct}`);
  }

  const addStringToken = (token, kind) => {
    if (token?.kind === SyntaxKind.StringLiteral) {
      imports.push({ kind, specifier: token.value });
      if (['react/jsx-runtime', 'react/jsx-dev-runtime'].includes(token.value)) {
        errors.push(`${filePath}: manual JSX runtime imports are forbidden in active source`);
      }
      return true;
    }
    return false;
  };

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];

    if (
      token.kind === SyntaxKind.ImportKeyword &&
      tokens[index + 1]?.kind === SyntaxKind.DotToken &&
      tokens[index + 2]?.value === 'meta' &&
      tokens[index + 3]?.kind === SyntaxKind.DotToken &&
      (tokens[index + 4]?.value === 'glob' || tokens[index + 4]?.value === 'globEager')
    ) {
      importMetaGlobCount += 1;
      errors.push(
        `${filePath}: import.meta.${tokens[index + 4].value} is forbidden in the active graph`,
      );
      continue;
    }

    if (token.kind === SyntaxKind.ImportKeyword) {
      if (tokens[index + 1]?.kind === SyntaxKind.OpenParenToken) {
        if (literalOnlyCallArgument(tokens, index + 1)) {
          addStringToken(tokens[index + 2], 'dynamic import');
        } else {
          nonLiteralDynamicImportCount += 1;
          errors.push(`${filePath}: dynamic import must contain exactly one string literal argument`);
        }
        continue;
      }

      if (addStringToken(tokens[index + 1], 'static import')) continue;
      for (let cursor = index + 1; cursor < tokens.length; cursor += 1) {
        if (
          tokens[cursor].kind === SyntaxKind.SemicolonToken ||
          tokens[cursor].kind === SyntaxKind.ImportKeyword ||
          tokens[cursor].kind === SyntaxKind.ExportKeyword
        ) {
          break;
        }
        if (tokens[cursor].kind === SyntaxKind.FromKeyword) {
          if (!addStringToken(tokens[cursor + 1], 'static import')) {
            errors.push(`${filePath}: static import must use a string literal module specifier`);
          }
          break;
        }
      }
      continue;
    }

    if (token.kind === SyntaxKind.ExportKeyword) {
      for (let cursor = index + 1; cursor < tokens.length; cursor += 1) {
        if (
          tokens[cursor].kind === SyntaxKind.SemicolonToken ||
          tokens[cursor].kind === SyntaxKind.ImportKeyword ||
          tokens[cursor].kind === SyntaxKind.ExportKeyword
        ) {
          break;
        }
        if (tokens[cursor].kind === SyntaxKind.FromKeyword) {
          if (!addStringToken(tokens[cursor + 1], 're-export')) {
            errors.push(`${filePath}: re-export must use a string literal module specifier`);
          }
          break;
        }
      }
      continue;
    }

    if (
      token.kind === SyntaxKind.Identifier &&
      token.value === 'require' &&
      tokens[index + 1]?.kind === SyntaxKind.OpenParenToken
    ) {
      if (literalOnlyCallArgument(tokens, index + 1)) {
        addStringToken(tokens[index + 2], 'require');
      } else {
        errors.push(`${filePath}: require must contain exactly one string literal argument`);
      }
    }
  }

  return {
    auditTokens,
    errors,
    importMetaGlobCount,
    imports,
    nonLiteralDynamicImportCount,
    prohibitedConstructs,
  };
}

function canonicalRowsSha256(rows) {
  return createHash('sha256').update(JSON.stringify(rows)).digest('hex').toUpperCase();
}

export function validateDenyInput(publicSourceDeny) {
  const errors = [];
  if (!Array.isArray(publicSourceDeny)) {
    return ['public SOURCE deny input must be an array'];
  }

  if (publicSourceDeny.length !== EXPECTED_PUBLIC_SOURCE_DENIES.length) {
    errors.push(
      `public SOURCE deny input must contain exactly ${EXPECTED_PUBLIC_SOURCE_DENIES.length} capabilities`,
    );
  }

  EXPECTED_PUBLIC_SOURCE_DENIES.forEach(([expectedDenyId, expectedCapability], index) => {
    const actual = publicSourceDeny[index];
    if (!actual) return;
    if (actual.denyId !== expectedDenyId || actual.capability !== expectedCapability) {
      errors.push(
        `public SOURCE deny row ${index + 1} must be ${expectedDenyId}/${expectedCapability}`,
      );
    }
    if (Object.keys(actual).join(',') !== 'denyId,capability,validationId,markers') {
      errors.push(`${expectedDenyId}: row fields/order must be denyId,capability,validationId,markers`);
    }
    const expectedValidationId = expectedDenyId.replace(/^P4-D-/u, 'P4-Z-');
    if (actual.validationId !== expectedValidationId) {
      errors.push(`${expectedDenyId}: validationId must be ${expectedValidationId}`);
    }
    if (!Array.isArray(actual.markers) || actual.markers.length === 0) {
      errors.push(`${expectedDenyId}: markers must be a non-empty array`);
      return;
    }
    if (actual.markers.some((marker) => typeof marker !== 'string' || marker.length === 0)) {
      errors.push(`${expectedDenyId}: every marker must be a non-empty string`);
    }
  });

  const denyIds = publicSourceDeny.map((row) => row?.denyId);
  if (new Set(denyIds).size !== denyIds.length) {
    errors.push('public SOURCE deny IDs must be unique');
  }

  const markerCount = publicSourceDeny.reduce(
    (count, row) => count + (Array.isArray(row?.markers) ? row.markers.length : 0),
    0,
  );
  if (markerCount !== EXPECTED_PUBLIC_SOURCE_MARKER_COUNT) {
    errors.push(
      `public SOURCE deny input must contain exactly ${EXPECTED_PUBLIC_SOURCE_MARKER_COUNT} markers`,
    );
  }
  if (canonicalRowsSha256(publicSourceDeny) !== EXPECTED_PUBLIC_SOURCE_ROWS_SHA256) {
    errors.push('public SOURCE deny rows/marker sets do not match the frozen P4 contract');
  }

  return errors;
}

export function validateDenyDocument(denyDocument) {
  if (!denyDocument || typeof denyDocument !== 'object' || Array.isArray(denyDocument)) {
    return ['public SOURCE deny document must be an object'];
  }
  const errors = [];
  if (
    Object.keys(denyDocument).join(',') !==
    'schemaVersion,markerSetVersion,kind,surface,isCapabilityManifest,snapshotMode,sourceReference,runtimeSourceRequired,capabilities'
  ) {
    errors.push('deny document fields/order do not match the frozen schema');
  }
  if (denyDocument.schemaVersion !== 2) errors.push('deny document schemaVersion must be 2');
  if (denyDocument.markerSetVersion !== 1) {
    errors.push('deny document markerSetVersion must be 1');
  }
  if (denyDocument.kind !== 'P4_PUBLIC_SOURCE_DENY_FROZEN_SNAPSHOT') {
    errors.push('deny document kind is invalid');
  }
  if (denyDocument.surface !== 'SOURCE') errors.push('deny document surface must be SOURCE');
  if (denyDocument.isCapabilityManifest !== false) {
    errors.push('deny document must explicitly state isCapabilityManifest=false');
  }
  if (
    denyDocument.sourceReference !==
    'project-governance/current/p4/05-public-capability-deny-audit.tsv'
  ) {
    errors.push('deny document provenance reference is invalid');
  }
  if (denyDocument.snapshotMode !== 'FROZEN_P7_1_003_NO_LIVE_SOURCE_DEPENDENCY') {
    errors.push('deny document snapshot mode is invalid');
  }
  if (denyDocument.runtimeSourceRequired !== false) {
    errors.push('deny document must not require its provenance source at runtime');
  }
  errors.push(...validateDenyInput(denyDocument.capabilities));
  return errors;
}

function closureFrom(entry, adjacency) {
  const visited = new Set();
  const pending = [entry];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || visited.has(current)) continue;
    visited.add(current);
    for (const dependency of adjacency.get(current) ?? []) pending.push(dependency);
  }
  return visited;
}

export function analyzeImportGraph({ files, publicSourceDeny, allowedExternalPackages }) {
  const normalizedFiles = new Map(
    [...files.entries()].map(([filePath, sourceText]) => [toPosixPath(filePath), sourceText]),
  );
  const allowedExternals = new Set(allowedExternalPackages ?? []);
  const errors = validateDenyInput(publicSourceDeny);
  const factsByFile = new Map();
  const adjacency = new Map();
  const edges = [];
  const prohibitedConstructs = [];
  let importMetaGlobCount = 0;
  let nonLiteralDynamicImportCount = 0;

  for (const [filePath, sourceText] of [...normalizedFiles.entries()].sort(([a], [b]) =>
    a.localeCompare(b),
  )) {
    const role = classifyActivePath(filePath);
    if (role === 'inactive') {
      errors.push(`${filePath}: file is outside the explicit active universe`);
      continue;
    }

    const facts = collectSourceFacts(filePath, sourceText);
    factsByFile.set(filePath, facts);
    adjacency.set(filePath, new Set());
    errors.push(...facts.errors);
    prohibitedConstructs.push(...facts.prohibitedConstructs);
    importMetaGlobCount += facts.importMetaGlobCount;
    nonLiteralDynamicImportCount += facts.nonLiteralDynamicImportCount;

    for (const imported of facts.imports) {
      const { specifier } = imported;
      if (!specifier.startsWith('.')) {
        if (specifier.startsWith('/') || !allowedExternals.has(externalPackageName(specifier))) {
          errors.push(`${filePath}: import ${specifier} is not an allowlisted external package`);
        }
        continue;
      }

      const resolved = resolveInternalImport(filePath, specifier, normalizedFiles);
      if (!resolved) {
        errors.push(
          `${filePath}: import ${specifier} is unresolved or reaches an inactive/legacy source`,
        );
        continue;
      }

      const targetRole = classifyActivePath(resolved);
      if (!ALLOWED_EDGE_TARGETS[role].has(targetRole)) {
        errors.push(`${filePath}: forbidden ${role} -> ${targetRole} edge to ${resolved}`);
      }
      adjacency.get(filePath).add(resolved);
      edges.push({ from: filePath, kind: imported.kind, to: resolved });
    }
  }

  for (const entry of Object.values(ENTRY_PATHS)) {
    if (!normalizedFiles.has(entry)) errors.push(`required entry is missing: ${entry}`);
  }

  const localClosure = closureFrom(ENTRY_PATHS.local, adjacency);
  const publicClosure = closureFrom(ENTRY_PATHS.public, adjacency);

  for (const required of REQUIRED_COMPOSITION_PATHS.local) {
    if (!localClosure.has(required)) errors.push(`local closure is missing ${required}`);
  }
  for (const required of REQUIRED_COMPOSITION_PATHS.public) {
    if (!publicClosure.has(required)) errors.push(`public closure is missing ${required}`);
  }

  for (const filePath of localClosure) {
    const role = classifyActivePath(filePath);
    if (role === 'public' || role === 'public-entry') {
      errors.push(`local closure reaches public source ${filePath}`);
    }
  }
  for (const filePath of publicClosure) {
    const role = classifyActivePath(filePath);
    if (role === 'local' || role === 'local-entry') {
      errors.push(`public closure reaches local source ${filePath}`);
    }
  }

  const reachable = new Set([...localClosure, ...publicClosure]);
  for (const filePath of normalizedFiles.keys()) {
    if (!reachable.has(filePath)) errors.push(`active source is unreachable from both entries: ${filePath}`);
  }

  const publicTargetEscapePatterns = [
    ['entrylocal', 'local entry'],
    ['localhtml', 'local HTML root'],
    ['srcuilocal', 'local source root'],
    ['uilocal', 'local source root'],
  ];
  for (const filePath of [...publicClosure].sort()) {
    const facts = factsByFile.get(filePath);
    if (!facts) continue;
    for (const token of facts.auditTokens) {
      const matched = publicTargetEscapePatterns.find(([pattern]) => token.includes(pattern));
      if (!matched) continue;
      const occurrence = {
        construct: `public target escape to ${matched[1]}`,
        file: filePath,
        index: `audit:${matched[0]}`,
      };
      prohibitedConstructs.push(occurrence);
      errors.push(`${filePath}: prohibited source-loading/code-generation construct ${occurrence.construct}`);
    }
  }

  const denyViolations = [];
  for (const filePath of [...publicClosure].sort()) {
    const facts = factsByFile.get(filePath);
    if (!facts) continue;
    for (const row of publicSourceDeny ?? []) {
      for (const marker of row.markers ?? []) {
        const normalizedMarker = normalizeAuditToken(marker);
        const matchedToken = [...facts.auditTokens].find((token) => token.includes(normalizedMarker));
        if (normalizedMarker && matchedToken) {
          denyViolations.push({
            capability: row.capability,
            denyId: row.denyId,
            file: filePath,
            marker,
            token: matchedToken,
          });
        }
      }
    }
  }

  for (const violation of denyViolations) {
    errors.push(
      `${violation.file}: public SOURCE deny ${violation.denyId} matched ${violation.marker}`,
    );
  }

  const filesByRole = Object.fromEntries(
    ['shared', 'local', 'public', 'local-entry', 'public-entry'].map((role) => [
      role,
      [...normalizedFiles.keys()].filter((filePath) => classifyActivePath(filePath) === role).sort(),
    ]),
  );

  return {
    schemaVersion: 1,
    taskId: 'P7.1-003',
    state: errors.length === 0 ? 'PASS' : 'FAIL',
    activeUniverse: {
      fileCount: normalizedFiles.size,
      files: [...normalizedFiles.keys()].sort(),
      filesByRole,
    },
    edges: edges.sort((a, b) => `${a.from}|${a.to}`.localeCompare(`${b.from}|${b.to}`)),
    closures: {
      local: [...localClosure].sort(),
      public: [...publicClosure].sort(),
    },
    publicSourceDeny: {
      expectedCount: EXPECTED_PUBLIC_SOURCE_DENIES.length,
      expectedMarkerCount: EXPECTED_PUBLIC_SOURCE_MARKER_COUNT,
      checkedCount: publicSourceDeny?.length ?? 0,
      checkedMarkerCount:
        publicSourceDeny?.reduce(
          (count, row) => count + (Array.isArray(row?.markers) ? row.markers.length : 0),
          0,
        ) ?? 0,
      rowsSha256: Array.isArray(publicSourceDeny)
        ? canonicalRowsSha256(publicSourceDeny)
        : null,
      violationCount: denyViolations.length,
      violations: denyViolations,
    },
    prohibitedConstructs: {
      count: prohibitedConstructs.length,
      importMetaGlobCount,
      nonLiteralDynamicImportCount,
      occurrences: prohibitedConstructs,
    },
    assertions: {
      allActiveFilesReachable: !errors.some((error) => error.includes('unreachable from both entries')),
      edgeRulesPass: !errors.some((error) => error.includes('forbidden') && error.includes('edge')),
      legacySourceReachable: errors.some((error) => error.includes('inactive/legacy source')),
      prohibitedConstructsAbsent: prohibitedConstructs.length === 0,
      publicSourceDenyPass: denyViolations.length === 0,
      publicToLocalEdges: edges.filter(
        ({ from, to }) =>
          ['public', 'public-entry'].includes(classifyActivePath(from)) &&
          ['local', 'local-entry'].includes(classifyActivePath(to)),
      ).length,
      sharedToTargetEdges: edges.filter(
        ({ from, to }) =>
          classifyActivePath(from) === 'shared' && classifyActivePath(to) !== 'shared',
      ).length,
    },
    errors: [...new Set(errors)].sort(),
  };
}

export function validateActiveFileSystemEntry(relativePath, stats, expectedKind) {
  if (stats.isSymbolicLink()) {
    throw new Error(`${relativePath}: symbolic links are forbidden in the active source universe`);
  }
  if (expectedKind === 'file' && !stats.isFile()) {
    throw new Error(`${relativePath}: required active entry must be a regular file`);
  }
  if (expectedKind === 'directory' && !stats.isDirectory()) {
    throw new Error(`${relativePath}: required active path must be a directory`);
  }
}

function assertFileSystemEntry(root, relativePath, expectedKind) {
  validateActiveFileSystemEntry(relativePath, lstatSync(resolve(root, relativePath)), expectedKind);
}

export function activeSourcePathDisposition(relativePath) {
  const extension = extname(relativePath).toLocaleLowerCase('en-US');
  if (SOURCE_EXTENSIONS.has(extension)) return 'source';
  if (SOURCE_LIKE_EXTENSIONS.has(extension)) {
    throw new Error(
      `${relativePath}: source-like extension ${extension} is forbidden; active sources must be .ts/.tsx`,
    );
  }
  return 'ignored-non-source';
}

function readActiveFiles(root) {
  const files = new Map();
  for (const entry of Object.values(ENTRY_PATHS)) {
    assertFileSystemEntry(root, entry, 'file');
    files.set(entry, readFileSync(resolve(root, entry), 'utf8'));
  }

  const walk = (relativeDirectory) => {
    assertFileSystemEntry(root, relativeDirectory, 'directory');
    const absoluteDirectory = resolve(root, relativeDirectory);
    for (const directoryEntry of readdirSync(absoluteDirectory, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const relativePath = toPosixPath(posix.join(relativeDirectory, directoryEntry.name));
      if (directoryEntry.isSymbolicLink()) {
        throw new Error(`${relativePath}: symbolic links are forbidden in the active source universe`);
      }
      if (directoryEntry.isDirectory()) {
        walk(relativePath);
        continue;
      }
      if (!directoryEntry.isFile()) {
        throw new Error(`${relativePath}: special filesystem entries are forbidden in the active source universe`);
      }
      const disposition = activeSourcePathDisposition(relativePath);
      if (disposition === 'source') {
        files.set(relativePath, readFileSync(resolve(root, relativePath), 'utf8'));
      }
    }
  };

  for (const directory of ACTIVE_DIRECTORIES) walk(directory);
  return files;
}

export function repositoryStaticContractFixture() {
  return new Map(Object.entries(STATIC_REPOSITORY_CONTRACT));
}

function isRepositoryStaticCandidate(filePath) {
  const baseName = posix.basename(toPosixPath(filePath));
  return /\.html$/iu.test(baseName) ||
    /^vite(?:\..+)?\.config\.(?:cjs|cts|js|mjs|mts|ts)$/iu.test(baseName);
}

export function validateRepositoryStaticFileUniverse(filePaths) {
  const candidates = [...filePaths]
    .map(toPosixPath)
    .filter(isRepositoryStaticCandidate)
    .sort();
  const expected = [...EXPECTED_ROOT_STATIC_FILES].sort();
  if (JSON.stringify(candidates) !== JSON.stringify(expected)) {
    return [
      `frontend root HTML/Vite file universe mismatch: expected ${expected.join(', ')}, got ${candidates.join(', ')}`,
    ];
  }
  return [];
}

export function validatePackageScripts(scripts) {
  const errors = [];
  const actualScriptNames = Object.keys(scripts ?? {}).sort();
  const expectedScriptNames = Object.keys(EXPECTED_PACKAGE_SCRIPTS).sort();
  if (JSON.stringify(actualScriptNames) !== JSON.stringify(expectedScriptNames)) {
    errors.push('package script universe does not match the frozen dual-target contract');
  }
  for (const [scriptName, expectedCommand] of Object.entries(EXPECTED_PACKAGE_SCRIPTS)) {
    if (scripts?.[scriptName] !== expectedCommand) {
      errors.push(`${scriptName}: package script does not match the frozen target contract`);
    }
  }
  return errors;
}

function readRepositoryStaticUniverse(root) {
  const candidates = [];
  const errors = [];
  const ignoredDirectories = new Set(['dist', 'node_modules']);
  const walk = (relativeDirectory = '') => {
    const absoluteDirectory = resolve(root, relativeDirectory);
    const entries = readdirSync(absoluteDirectory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name, 'en-US'));
    for (const entry of entries) {
      const relativePath = toPosixPath(posix.join(relativeDirectory, entry.name));
      if (entry.isSymbolicLink()) {
        errors.push(`${relativePath}: symlink is forbidden in the frontend build-root universe`);
        continue;
      }
      if (entry.isDirectory()) {
        if (!ignoredDirectories.has(entry.name)) walk(relativePath);
        continue;
      }
      if (entry.isFile() && isRepositoryStaticCandidate(relativePath)) candidates.push(relativePath);
    }
  };
  walk();
  return { candidates, errors };
}

function validateHtmlModuleRoot(filePath, sourceText, expectedEntry) {
  const errors = [];
  const scriptTags = [...sourceText.matchAll(/<script\b([^>]*)>/giu)];
  const moduleScripts = scriptTags.filter((match) => /\btype\s*=\s*["']module["']/iu.test(match[1]));
  if (scriptTags.length !== 1 || moduleScripts.length !== 1) {
    errors.push(`${filePath}: must contain exactly one script and it must be type=module`);
    return errors;
  }
  const sourceMatch = /\bsrc\s*=\s*["']([^"']+)["']/iu.exec(moduleScripts[0][1]);
  if (sourceMatch?.[1] !== expectedEntry) {
    errors.push(`${filePath}: module script must point only to ${expectedEntry}`);
  }
  return errors;
}

export function validateRepositoryStaticContracts(textFiles) {
  const errors = validateRepositoryStaticFileUniverse(textFiles.keys());
  for (const [filePath, expectedText] of Object.entries(STATIC_REPOSITORY_CONTRACT)) {
    const actualText = textFiles.get(filePath);
    if (typeof actualText !== 'string') {
      errors.push(`${filePath}: required static build contract file is missing`);
    } else if (normalizeText(actualText) !== normalizeText(expectedText)) {
      errors.push(`${filePath}: content drifted from the frozen dual-target build contract`);
    }
  }
  const localHtml = textFiles.get('local.html');
  if (typeof localHtml === 'string') {
    errors.push(...validateHtmlModuleRoot('local.html', localHtml, '/src/entry.local.tsx'));
  }
  const publicHtml = textFiles.get('public.html');
  if (typeof publicHtml === 'string') {
    errors.push(...validateHtmlModuleRoot('public.html', publicHtml, '/src/entry.public.tsx'));
  }
  return errors;
}

function readRepositoryInputs(root) {
  const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
  const repositoryRoot = resolve(root, '..');
  const denyDocument = JSON.parse(
    readFileSync(resolve(repositoryRoot, 'tools/architecture/p4-public-source-deny.json'), 'utf8'),
  );
  const textFiles = new Map(
    Object.keys(STATIC_REPOSITORY_CONTRACT).map((filePath) => [
      filePath,
      readFileSync(resolve(root, filePath), 'utf8'),
    ]),
  );
  const staticUniverse = readRepositoryStaticUniverse(root);
  const metadataErrors = [
    ...validateDenyDocument(denyDocument),
    ...validateRepositoryStaticContracts(textFiles),
    ...staticUniverse.errors,
    ...validateRepositoryStaticFileUniverse(staticUniverse.candidates),
  ];

  metadataErrors.push(...validatePackageScripts(packageJson.scripts));

  return {
    allowedExternalPackages: new Set(Object.keys(packageJson.dependencies ?? {})),
    files: readActiveFiles(root),
    metadataErrors,
    publicSourceDeny: denyDocument.capabilities,
  };
}

export function verifyRepositoryImportGraph(root = frontendRoot) {
  const inputs = readRepositoryInputs(root);
  const report = analyzeImportGraph(inputs);
  report.repositoryContract = {
    staticFileCount: Object.keys(STATIC_REPOSITORY_CONTRACT).length,
    metadataErrorCount: inputs.metadataErrors.length,
    pass: inputs.metadataErrors.length === 0,
  };
  if (inputs.metadataErrors.length > 0) {
    report.errors.push(...inputs.metadataErrors);
    report.errors = [...new Set(report.errors)].sort();
    report.state = 'FAIL';
  }
  return report;
}

function renderHumanReport(report) {
  if (report.state === 'PASS') {
    return [
      'P7.1-003 frontend import graph: PASS',
      `active files: ${report.activeUniverse.fileCount}`,
      `internal edges: ${report.edges.length}`,
      `local closure: ${report.closures.local.length}`,
      `public closure: ${report.closures.public.length}`,
      `public SOURCE denies: ${report.publicSourceDeny.checkedCount}/${report.publicSourceDeny.expectedCount}`,
      `public SOURCE markers: ${report.publicSourceDeny.checkedMarkerCount}/${report.publicSourceDeny.expectedMarkerCount}`,
    ].join('\n');
  }
  return ['P7.1-003 frontend import graph: FAIL', ...report.errors.map((error) => `- ${error}`)].join(
    '\n',
  );
}

function isMainModule() {
  if (!process.argv[1]) return false;
  return pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const unknownArguments = args.filter((argument) => argument !== '--json');
  try {
    const report = unknownArguments.length > 0
      ? {
          schemaVersion: 1,
          taskId: 'P7.1-003',
          state: 'FAIL',
          errors: [`unknown argument(s): ${unknownArguments.join(', ')}`],
        }
      : verifyRepositoryImportGraph();
    process.stdout.write(
      jsonMode ? `${JSON.stringify(report, null, 2)}\n` : `${renderHumanReport(report)}\n`,
    );
    if (report.state !== 'PASS') process.exitCode = 1;
  } catch (error) {
    const failure = {
      schemaVersion: 1,
      taskId: 'P7.1-003',
      state: 'FAIL',
      errors: [error instanceof Error ? error.message : String(error)],
    };
    process.stdout.write(
      jsonMode ? `${JSON.stringify(failure, null, 2)}\n` : `${renderHumanReport(failure)}\n`,
    );
    process.exitCode = 1;
  }
}
