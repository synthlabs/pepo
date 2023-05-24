module.exports = {
	root: true,
	parser: '@typescript-eslint/parser',
	extends: ['eslint:recommended', 'plugin:@typescript-eslint/recommended', 'prettier'],
	plugins: ['svelte3', '@typescript-eslint'],
	ignorePatterns: ['*.cjs'],
	overrides: [{ files: ['*.svelte'], processor: 'svelte3/svelte3' }],
	settings: {
		'svelte3/typescript': () => require('typescript'),
		'svelte3/ignore-warnings': ({ code }) => code.includes('a11y'),
	},
	parserOptions: {
		sourceType: 'module',
		ecmaVersion: 2020
	},
	env: {
		browser: true,
		es2017: true,
		node: true
	},
	rules: {
		'a11y-aria-attributes': 'off',
		'a11y-incorrect-aria-attribute-type': 'off',
		'a11y-unknown-aria-attribute': 'off',
		'a11y-hidden': 'off',
		'a11y-misplaced-role': 'off',
		'a11y-unknown-role': 'off',
		'a11y-no-abstract-role': 'off',
		'a11y-no-redundant-roles': 'off',
		'a11y-role-has-required-aria-props': 'off',
		'a11y-accesskey': 'off',
		'a11y-autofocus': 'off',
		'a11y-misplaced-scope': 'off',
		'a11y-positive-tabindex': 'off',
		'a11y-invalid-attribute': 'off',
		'a11y-missing-attribute': 'off',
		'a11y-img-redundant-alt': 'off',
		'a11y-label-has-associated-control': 'off',
		'a11y-media-has-caption': 'off',
		'a11y-distracting-elements': 'off',
		'a11y-structure': 'off',
		'a11y-mouse-events-have-key-events': 'off',
		'a11y-missing-content': 'off',
		'a11y-click-events-have-key-events': 'off',
		'a11y-no-noninteractive-tabindex': 'off',
	}
};
