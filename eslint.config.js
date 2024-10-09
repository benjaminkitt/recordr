// @ts-check
import eslint from '@eslint/js';
import eslintPluginSvelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';
import svelteConfig from './svelte.config.js';
import globals from 'globals';
import prettierPlugin from 'eslint-plugin-prettier';
import { default as eslintPluginPrettierRecommended } from 'eslint-plugin-prettier/recommended';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...eslintPluginSvelte.configs['flat/prettier'],
  eslintPluginPrettierRecommended,
  {
    plugins: {
      prettier: prettierPlugin,
    },
    rules: {
      'prettier/prettier': 'error',
    },
  },
  {
    files: ['**/*.svelte', '*.svelte'],
    languageOptions: {
      globals: {
        ...globals.browser,
      },
      parserOptions: {
        svelteConfig,
        parser: '@typescript-eslint/parser',
      },
    },
  },
  {
    files: ['src/**/*.ts', 'src/**/*.tsx'],
    languageOptions: {
      parserOptions: {
        project: './tsconfig.json',
        parser: '@typescript-eslint/parser',
      },
    },
    plugins: {
      '@typescript-eslint': tseslint.plugin,
    },
  }
);
