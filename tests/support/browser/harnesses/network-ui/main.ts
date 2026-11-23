import { mount } from 'svelte';
import './native.svelte';
import Fixture from './Fixture.svelte';
import '../../../../../src/app.css';

mount(Fixture, { target: document.getElementById('app')! });
