<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';

	let text = $state('hi');
</script>

<div class="container">
	<h1>{text}</h1>
</div>
