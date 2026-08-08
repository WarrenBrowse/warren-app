import react from 'eslint-plugin-react';
import reactcompiler from 'eslint-plugin-react-compiler';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

import workspaceConfig from '../../eslint.config.mjs';

// A `messages.*gettext*` call evaluated at module load runs before
// `loadTranslations` has filled the catalogue, so it returns the English
// msgid forever, in every locale. It shipped that way once: the update
// screen said "Download complete!" in the middle of a French UI, because a
// module-level `const translations = { x: messages.pgettext(...) }` captured
// the untranslated strings at import time. Translation lookups belong inside
// a function (a hook, a getter, a callback), where they run at render time.
const lazyGettextPlugin = {
  rules: {
    'no-module-scope-gettext': {
      meta: {
        type: 'problem',
        schema: [],
        messages: {
          eager:
            'messages.{{name}}() at module scope runs before the locale is loaded and pins the English msgid. Wrap it in a function or a getter so it runs at render time.',
        },
      },
      create(context) {
        return {
          // eslint-disable-next-line @typescript-eslint/naming-convention
          CallExpression(node) {
            const callee = node.callee;
            const isGettext =
              callee.type === 'MemberExpression' &&
              !callee.computed &&
              callee.object.type === 'Identifier' &&
              callee.object.name === 'messages' &&
              callee.property.type === 'Identifier' &&
              /gettext$/.test(callee.property.name);
            if (!isGettext) {
              return;
            }
            for (let ancestor = node.parent; ancestor; ancestor = ancestor.parent) {
              if (
                ancestor.type === 'FunctionDeclaration' ||
                ancestor.type === 'FunctionExpression' ||
                ancestor.type === 'ArrowFunctionExpression'
              ) {
                return;
              }
            }
            context.report({ node, messageId: 'eager', data: { name: callee.property.name } });
          },
        };
      },
    },
  },
};

export default [
  ...workspaceConfig,
  react.configs.flat.recommended,
  { ignores: ['build/', 'build-standalone/'] },
  {
    files: ['**/*'],
    ignores: ['src/renderer/'],
    languageOptions: { globals: globals.node },
  },
  {
    files: ['src/renderer/'],
    languageOptions: { globals: globals.browser },
  },
  {
    settings: {
      react: {
        createClass: 'createReactClass',
        pragma: 'React',
        version: 'detect',
      },
    },
  },
  {
    files: ['**/*.{js,mjs,ts,tsx}'],
    plugins: {
      'react-hooks': reactHooks,
      'react-compiler': reactcompiler,
    },
    rules: {
      'react/jsx-no-bind': 'error',
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'error',
      'react-compiler/react-compiler': 'error',
      'react/prop-types': 'off',
      'react/react-in-jsx-scope': 'off',
    },
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    plugins: { 'lazy-gettext': lazyGettextPlugin },
    rules: { 'lazy-gettext/no-module-scope-gettext': 'error' },
  },
  {
    files: ['test/**/*.spec.ts'],
    rules: { '@typescript-eslint/no-unused-expressions': 'off' },
  },
  {
    files: ['tasks/*', 'scripts/*'],
    rules: { '@typescript-eslint/no-require-imports': 'off' },
  },
  {
    // The renderer is sandboxed and has no node. Anything that pulls the
    // `electron` package into its bundle makes Vite emit `__dirname`, and the
    // bundle then throws `__dirname is not defined` on load: the React app never
    // mounts, the window paints nothing, and clicking the tray icon looks like a
    // dead app. It shipped that way in 1.1.5. IPC belongs on the
    // contextBridge-exposed `window.ipc`.
    //
    // `preload.ts` is exempt (it is a separate bundle that runs WITH node, and
    // it is what exposes `window.ipc` in the first place), as is the channel
    // module itself.
    files: ['src/renderer/**/*.{ts,tsx}'],
    ignores: ['src/renderer/preload.ts', 'src/renderer/lib/ipc-event-channel.ts'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: 'electron',
              message:
                'The sandboxed renderer has no node: use the contextBridge (window.ipc). Importing electron here breaks the whole window with "__dirname is not defined".',
            },
          ],
          patterns: [
            {
              group: ['**/lib/ipc-event-channel'],
              message:
                'ipc-event-channel imports electron, which breaks the sandboxed renderer bundle: use window.ipc instead.',
            },
          ],
        },
      ],
    },
  },
];
