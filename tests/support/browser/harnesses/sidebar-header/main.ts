import { mount } from 'svelte';
import Preview from './Preview.svelte';
import '../../../../../src/app.css';

const params = new URLSearchParams(location.search);
document.documentElement.classList.toggle('dark', params.get('theme') !== 'light');
mount(Preview, {
	target: document.body,
	props: {
		open: params.get('open') !== 'false',
		language: params.has('no-language') ? null : 'English',
		tags: !params.has('no-tags'),
		long: params.has('long')
	}
});
