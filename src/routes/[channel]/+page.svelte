<script lang="ts">
	import { page } from '$app/stores';
	import Logger from '$lib/logger/log';

	Logger.debug($page);
</script>

<pre>
    {JSON.stringify($page, null, 2)}
</pre>
